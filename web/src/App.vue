<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue';
import { VectorEngine } from './pkg/engine';
import { aiService, type ModelStatus } from './ai';
import LayerItem from './components/LayerItem.vue';
import { 
    MousePointer2, Square, Circle, PenTool, Crop, 
    Star, Hexagon, Pipette, Type, Upload,
    Trash2, Copy, BringToFront, SendToBack, ChevronUp, ChevronDown,
    Pencil, Eraser, Hand, Search, RotateCw, PaintBucket, Brush,
    Zap, Plus, History, Settings, Wand2, Stamp, Sparkles
} from 'lucide-vue-next';

type Tool = 'select' | 'rect' | 'circle' | 'image' | 'bezier' | 'crop' | 'star' | 'poly' | 'eyedropper' | 'text' | 'pencil' | 'brush' | 'eraser' | 'hand' | 'zoom' | 'rotate' | 'gradient' | 'vectorize' | 'adjustment' | 'magic_wand' | 'clone_stamp';

const canvas = ref<HTMLCanvasElement | null>(null);
const canvasContainer = ref<HTMLElement | null>(null);
const chatHistory = ref<HTMLElement | null>(null);
const engine = ref<VectorEngine | null>(null);
const engineLoadError = ref<string | null>(null);

// Brush State
const brushes = ref<any[]>([]);
const selectedBrushId = ref(1);
const brushColor = ref('#000000');
const brushState = ref({
    isDrawing: false,
    points: [] as { x: number, y: number, pressure: number }[],
    currentObjId: -1,
});

function updateBrushById(brush: any) {
    if (!engine.value || !brush) return;
    // Ensure numeric values are actually numbers (not strings from inputs)
    const sanitizedBrush = {
        ...brush,
        size: Number(brush.size),
        spacing: Number(brush.spacing),
        smoothing: Number(brush.smoothing),
        scatter: Number(brush.scatter),
        rotation_jitter: Number(brush.rotation_jitter),
        min_size_fraction: Number(brush.min_size_fraction || 0.2)
    };
    executeCommand({
        action: 'update_brush',
        params: sanitizedBrush
    });
}

function updateBrush() {
    updateBrushById(activeBrush.value);
}

const activeBrush = computed({
    get: () => brushes.value.find(b => b.id === selectedBrushId.value),
    set: (val) => {
        const idx = brushes.value.findIndex(b => b.id === selectedBrushId.value);
        if (idx !== -1) brushes.value[idx] = val;
    }
});

const chatInput = ref('');
const messages = ref<{ role: string, content: string }[]>([]);
const fileInput = ref<HTMLInputElement | null>(null);
const openImageInput = ref<HTMLInputElement | null>(null);
const aiStatus = ref<ModelStatus>({ status: 'idle', message: 'AI ready to load' });

// Tooltip State
const tooltip = ref({ show: false, text: '', x: 0, y: 0 });
function showTooltip(e: MouseEvent, text: string) {
    tooltip.value = {
        show: true,
        text,
        x: e.clientX,
        y: e.clientY
    };
}
function moveTooltip(e: MouseEvent) {
    tooltip.value.x = e.clientX;
    tooltip.value.y = e.clientY;
}
function hideTooltip() {
    tooltip.value.show = false;
}

// State
const activeTool = ref<Tool>('select');
const showShapesMenu = ref(false);
const snapMode = ref(true);
const objects = ref<any[]>([]);
const history = ref<string[]>([]);
const imageMap = new Map<number, HTMLImageElement>();
const selectedIds = ref<number[]>([]); // Changed to array
const isDragging = ref(false);
const dragStart = ref({ x: 0, y: 0 });
const lastMousePos = ref({ x: 0, y: 0 });
// Store initial state for all selected objects during drag
const initialObjectsState = ref<Map<number, { x: number, y: number, width: number, height: number, rotation: number }>>(new Map());
const needsRender = ref(true);

const selectionBox = ref<{ x: number, y: number, w: number, h: number } | null>(null);
const activeHandle = ref<{ id: number, type: string } | null>(null);


function booleanOp(operation: string) {
  executeCommand({
    action: 'boolean_operation',
    params: {
      operation,
      ids: selectedIds.value
    }
  });
}

function toggleMask() {
  if (!selectedObject.value) return;
  const newValue = !selectedObject.value.is_mask;
  updateSelected('is_mask', newValue);
  
  if (newValue) {
    // If it becomes a mask, we might want to attach it to the layer above?
    // In our sequential rendering, we'll just let the user set mask_id on other layers.
  }
}

function createAdjustment() {
  if (!engine.value) return;
  const res = executeCommand({
    action: 'add',
    params: {
      type: 'Adjustment',
      x: 0,
      y: 0,
      width: artboard.value.width,
      height: artboard.value.height,
      save_undo: true
    }
  });
  if (res && res.id) {
    executeCommand({ action: 'select', params: { id: res.id } });
  }
}

watch(activeTool, (newTool) => {
  if (newTool === 'adjustment') {
    createAdjustment();
    activeTool.value = 'select'; // Switch back to select to tweak properties
  }
  needsRender.value = true;
});

watch(selectedIds, (newIds) => {
    needsRender.value = true;
    // Auto-enter bezier edit mode if a single path is selected
    if (newIds.length === 1 && (activeTool.value === 'select' || activeTool.value === 'bezier' || activeTool.value === 'brush')) {
        const obj = objects.value.find(o => o.id === newIds[0]);
        if (obj && obj.shape_type === 'Path') {
            bezierState.value.isEditing = true;
            bezierState.value.currentObjId = obj.id;
            bezierState.value.points = parsePathData(obj.path_data, obj.x, obj.y);
        } else {
            bezierState.value.isEditing = false;
            bezierState.value.points = [];
            bezierState.value.currentObjId = -1;
        }
    } else {
        bezierState.value.isEditing = false;
        bezierState.value.points = [];
        bezierState.value.currentObjId = -1;
    }
}, { deep: true });

watch(messages, () => {
    nextTick(() => {
        if (chatHistory.value) {
            chatHistory.value.scrollTop = chatHistory.value.scrollHeight;
        }
    });
}, { deep: true });

const vectorizeThreshold = ref(128);
const lastVectorizedResult = ref<{ sourceId: number, pathId: number } | null>(null);
const eraserSize = ref(20);
const magicWandTolerance = ref(30);
const cloneSource = ref<{ x: number, y: number } | null>(null);
const cloneSize = ref(20);

function updateImageFromEngine(id: number) {
  if (!engine.value) return;
  const pixels = engine.value.get_image_rgba(id);
  const width = engine.value.get_image_width(id);
  const height = engine.value.get_image_height(id);
  if (pixels && width && height) {
      const tempCanvas = document.createElement('canvas');
      tempCanvas.width = width;
      tempCanvas.height = height;
      const tCtx = tempCanvas.getContext('2d');
      if (tCtx) {
          const imageData = new ImageData(new Uint8ClampedArray(pixels), width, height);
          tCtx.putImageData(imageData, 0, 0);
          engine.value.set_image_object(id, tempCanvas);
          needsRender.value = true;
      }
  }
}

function startGuideDrag(orientation: 'horizontal' | 'vertical') {
    const handleMove = (e: MouseEvent) => {
        const rect = canvas.value?.getBoundingClientRect();
        if (!rect) return;
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        const worldPos = screenToWorld(x, y);
        
        // Render a temporary guide or just wait for up
    };
    const handleUp = (e: MouseEvent) => {
        window.removeEventListener('mousemove', handleMove);
        window.removeEventListener('mouseup', handleUp);
        
        const rect = canvas.value?.getBoundingClientRect();
        if (!rect) return;
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        const worldPos = screenToWorld(x, y);
        
        executeCommand({
            action: 'add_guide',
            params: {
                orientation,
                position: orientation === 'horizontal' ? worldPos.y : worldPos.x
            }
        });
    };
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
}

function vectorizeImage(saveUndo: boolean = true) {
    console.log("Frontend: Requesting vectorization with threshold:", vectorizeThreshold.value, "saveUndo:", saveUndo);
    const id = targetImageId.value;
    if (id === -1) {
        console.warn("Frontend: No image found for vectorization");
        return;
    }

    const obj = objects.value.find(o => o.id === id);
    if (!obj || obj.shape_type !== 'Image') {
        console.warn("Frontend: Selected object is not an image");
        return;
    }

    // 1. If we are re-vectorizing, remove the previous result first
    if (lastVectorizedResult.value && lastVectorizedResult.value.sourceId === id) {
        if (objects.value.some(o => o.id === lastVectorizedResult.value?.pathId)) {
            executeCommand({ 
                action: 'delete', 
                params: { 
                    id: lastVectorizedResult.value.pathId,
                    save_undo: false // Don't save undo for the intermediate delete
                } 
            });
        }
    }

    // 2. Execute vectorization
    const res = executeCommand({
        action: 'vectorize',
        params: {
            id: id,
            threshold: vectorizeThreshold.value,
            save_undo: saveUndo
        }
    });
    
    console.log("Frontend: Vectorization result:", res);
    if (res && res.id) {
        lastVectorizedResult.value = { sourceId: id, pathId: res.id };
        // Select the new path so it's visible
        executeCommand({ action: 'select', params: { id: res.id } });
    }
}

let vectorizeTimeout: any = null;
watch(vectorizeThreshold, () => {
    if (activeTool.value !== 'vectorize') return;
    
    if (vectorizeTimeout) clearTimeout(vectorizeTimeout);
    vectorizeTimeout = setTimeout(() => {
        vectorizeImage(false);
    }, 50); // Small debounce for smoothness
});

const viewport = ref({ x: 0, y: 0, zoom: 1.0 });
const isPanning = ref(false);
const isSpacePressed = ref(false);
const artboard = ref({ width: 800, height: 600, background: '#ffffff' });
const clipToArtboard = ref(false);
const showDocProps = ref(false);

function screenToWorld(x: number, y: number) {
    if (!canvas.value) return { x, y };
    // Get the actual screen position of the canvas element
    // Since the canvas is nested and offset by rulers, we need its client rect
    const rect = canvas.value.getBoundingClientRect();
    const cx = x - rect.left;
    const cy = y - rect.top;
    
    return {
        x: (cx - viewport.value.x) / viewport.value.zoom,
        y: (cy - viewport.value.y) / viewport.value.zoom
    };
}

const cropState = ref({
    isCropping: false,
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
});

const gradientState = ref({
    isDragging: false,
    dragType: null as 'start' | 'end' | 'stop' | null,
    dragIndex: -1,
    activeStopIndex: -1,
});

function worldToLocal(obj: any, wx: number, wy: number) {
    const cx = obj.x + obj.width / 2;
    const cy = obj.y + obj.height / 2;
    const dx = wx - cx;
    const dy = wy - cy;
    const cos_r = Math.cos(-obj.rotation);
    const sin_r = Math.sin(-obj.rotation);
    // Rotate back
    const rx = dx * cos_r - dy * sin_r;
    const ry = dx * sin_r + dy * cos_r;
    // Translate to top-left origin
    return {
        x: rx + obj.width / 2,
        y: ry + obj.height / 2
    };
}

function localToWorld(obj: any, lx: number, ly: number) {
    // Translate from top-left origin to center-relative
    const rx = lx - obj.width / 2;
    const ry = ly - obj.height / 2;
    
    const cos_r = Math.cos(obj.rotation);
    const sin_r = Math.sin(obj.rotation);
    
    // Rotate
    const dx = rx * cos_r - ry * sin_r;
    const dy = rx * sin_r + ry * cos_r;
    
    // Translate to world center
    const cx = obj.x + obj.width / 2;
    const cy = obj.y + obj.height / 2;
    
    return {
        x: cx + dx,
        y: cy + dy
    };
}

function safeColor(color: string): string {
    if (!color || color === 'transparent' || !/^#[0-9A-F]{6}$/i.test(color)) {
        return '#000000';
    }
    return color;
}

// Bezier State
interface BezierPoint {
    x: number;
    y: number;
    cin: { x: number, y: number };
    cout: { x: number, y: number };
}

const bezierState = ref({
    isDrawing: false,
    isEditing: false,
    points: [] as BezierPoint[],
    currentObjId: -1,
    isSnapped: false,
    isClosing: false,
    mousePoint: null as { x: number, y: number } | null,
    dragIndex: -1,
    dragType: null as 'anchor' | 'cin' | 'cout' | null,
});

function parsePathData(d: string, offsetX: number, offsetY: number): BezierPoint[] {
    const points: BezierPoint[] = [];
    if (!d) return points;

    // Normalize spaces and split
    const commands = d.replace(/([a-zA-Z])/g, ' $1 ').trim().split(/\s+(?=[a-zA-Z])/);
    
    let currentPoint: BezierPoint | null = null;
    let firstPoint: BezierPoint | null = null;

    commands.forEach(cmdStr => {
        const type = cmdStr.trim()[0];
        const args = cmdStr.substring(1).trim().split(/[\s,]+/).map(parseFloat);

        if (type === 'M') {
            currentPoint = {
                x: args[0] + offsetX,
                y: args[1] + offsetY,
                cin: { x: args[0] + offsetX, y: args[1] + offsetY },
                cout: { x: args[0] + offsetX, y: args[1] + offsetY }
            };
            points.push(currentPoint);
            firstPoint = currentPoint;
        } else if (type === 'L') {
            currentPoint = {
                x: args[0] + offsetX,
                y: args[1] + offsetY,
                cin: { x: args[0] + offsetX, y: args[1] + offsetY },
                cout: { x: args[0] + offsetX, y: args[1] + offsetY }
            };
            points.push(currentPoint);
        } else if (type === 'C') {
            // C x1 y1, x2 y2, x y
            // prev.cout = x1, y1
            // curr.cin = x2, y2
            // curr = x, y
            if (points.length > 0) {
                const prev = points[points.length - 1];
                prev.cout = { x: args[0] + offsetX, y: args[1] + offsetY };
            }
            
            currentPoint = {
                x: args[4] + offsetX,
                y: args[5] + offsetY,
                cin: { x: args[2] + offsetX, y: args[3] + offsetY },
                cout: { x: args[4] + offsetX, y: args[5] + offsetY } // Default cout = anchor
            };
            points.push(currentPoint);
        } else if (type === 'Z') {
             // Closed.
             // Usually implies the last point connects to first.
             // getPathString adds a C command for this.
             // If we just parsed a C that ends at firstPoint, we might have a duplicate.
             // Let's check dist
             if (currentPoint && firstPoint) {
                 const dx = currentPoint.x - firstPoint.x;
                 const dy = currentPoint.y - firstPoint.y;
                 if (Math.abs(dx) < 0.1 && Math.abs(dy) < 0.1) {
                     // The last point IS the first point (geometrically)
                     // But strictly speaking, in our point list, we want unique points.
                     // The loop is formed by connecting last to first.
                     // If the SVG explicitly had a point at the start, we might have added it.
                     // Let's remove the last point if it's identical to first
                     points.pop();
                     // And fix the cout of the NEW last point (which was the prev point)
                     // The C command we just popped had control points.
                     // C cp1 cp2 P_first
                     // The prev point (now last) should have cout = cp1
                     // The first point should have cin = cp2
                     
                     // We need to retrieve the args of the C command that created the popped point.
                     // But we just popped it.
                     // This is getting complicated to reverse-engineer perfectly without lookahead/lookbehind.
                     
                     // Alternative: Just mark isClosing = true if we detect Z?
                     // But we need to set the state correctly.
                 }
             }
             bezierState.value.isClosing = true;
        }
    });
    
    // If Z was present, we need to wire up the loop control points if they exist.
    // The C command before Z: C cp1 cp2 P_first
    // We processed it as: prev.cout=cp1, P_dup.cin=cp2, P_dup=P_first.
    // If we popped P_dup:
    // We need prev.cout (already set correctly to cp1).
    // We need P_first.cin = cp2.
    // But P_first is points[0].
    
    // Let's refine the loop handling:
    // If we detect Z, we assume the path is closed.
    // If the last command was C, we check if it closed the loop.
    
    // Actually, simpler approach for now: 
    // Just parse points. If last point is same as first, merge them.
    if (points.length > 1) {
        const first = points[0];
        const last = points[points.length - 1];
        if (Math.abs(first.x - last.x) < 0.01 && Math.abs(first.y - last.y) < 0.01) {
             // It's a closed loop physically
             first.cin = last.cin; // Transfer cin from last to first
             // The cout of the point BEFORE last is already set correctly.
             points.pop();
             bezierState.value.isClosing = true;
        }
    }

    return points;
}


const pencilState = ref({
    isDrawing: false,
    points: [] as { x: number, y: number }[],
    currentObjId: -1,
});

function getPencilPathString(points: { x: number, y: number }[], offset = { x: 0, y: 0 }): string {
    if (points.length === 0) return '';
    const ox = offset.x;
    const oy = offset.y;
    let d = `M ${points[0].x - ox} ${points[0].y - oy}`;
    for (let i = 1; i < points.length; i++) {
        d += ` L ${points[i].x - ox} ${points[i].y - oy}`;
    }
    return d;
}

function getPathString(points: BezierPoint[], isClosed: boolean = false, previewPoint: {x: number, y: number} | null = null, offset = { x: 0, y: 0 }): string {
    if (points.length === 0) return '';
    const ox = offset.x;
    const oy = offset.y;
    
    let d = `M ${points[0].x - ox} ${points[0].y - oy}`;
    
    for (let i = 1; i < points.length; i++) {
        const prev = points[i-1];
        const curr = points[i];
        d += ` C ${prev.cout.x - ox} ${prev.cout.y - oy}, ${curr.cin.x - ox} ${curr.cin.y - oy}, ${curr.x - ox} ${curr.y - oy}`;
    }

    if (previewPoint && points.length > 0 && !isClosed) {
        const prev = points[points.length - 1];
        d += ` C ${prev.cout.x - ox} ${prev.cout.y - oy}, ${previewPoint.x - ox} ${previewPoint.y - oy}, ${previewPoint.x - ox} ${previewPoint.y - oy}`;
    }

    if (isClosed && points.length > 1) {
        const prev = points[points.length - 1];
        const curr = points[0];
        d += ` C ${prev.cout.x - ox} ${prev.cout.y - oy}, ${curr.cin.x - ox} ${curr.cin.y - oy}, ${curr.x - ox} ${curr.y - oy} Z`;
    }
    return d;
}

const hasImage = computed(() => {
    return objects.value.some(o => o.shape_type === 'Image');
});

const selectedObjects = computed(() => {
    return objects.value.filter(o => selectedIds.value.includes(o.id));
});

// Primary object for property editing (usually the last selected)
const selectedObject = computed(() => {
    if (selectedObjects.value.length === 0) return null;
    return selectedObjects.value[selectedObjects.value.length - 1];
});

const targetImageId = computed(() => {
    // 1. If an image is selected, use it
    if (selectedObject.value && selectedObject.value.shape_type === 'Image') {
        return selectedObject.value.id;
    }
    // 2. If we just vectorized something, keep targeting that source
    if (lastVectorizedResult.value && objects.value.some(o => o.id === (lastVectorizedResult.value as any).sourceId)) {
        return (lastVectorizedResult.value as any).sourceId;
    }
    // 3. Fallback to bottommost locked image
    const lockedImages = objects.value.filter(o => o.shape_type === 'Image' && o.locked);
    if (lockedImages.length > 0) {
        return lockedImages[0].id;
    }
    return -1;
});

const activeStopWorldPos = computed(() => {
    if (activeTool.value !== 'gradient' || !selectedObject.value || !selectedObject.value.fill_gradient || gradientState.value.activeStopIndex === -1) {
        return null;
    }
    const obj = selectedObject.value;
    const grad = obj.fill_gradient;
    const stop = grad.stops[gradientState.value.activeStopIndex];
    if (!stop) return null;

    const p1 = localToWorld(obj, grad.x1, grad.y1);
    const p2 = localToWorld(obj, grad.x2, grad.y2);
    
    const wx = p1.x + (p2.x - p1.x) * stop.offset;
    const wy = p1.y + (p2.y - p1.y) * stop.offset;

    // Convert world to screen
    return {
        x: wx * viewport.value.zoom + viewport.value.x,
        y: wy * viewport.value.zoom + viewport.value.y
    };
});

async function importBrushTip() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/*';
    input.onchange = async (e: any) => {
        const file = e.target.files[0];
        if (!file) return;
        
        const reader = new FileReader();
        reader.onload = async (re) => {
            const img = new Image();
            img.onload = () => {
                if (engine.value) {
                    const tipId = `tip_${Date.now()}`;
                    engine.value.register_brush_tip(tipId, img);
                    
                    // Add a new brush with this tip
                    const newBrush = {
                        id: 0,
                        name: file.name.split('.')[0],
                        tip: { Image: { image_id: tipId } },
                        size: 50,
                        spacing: 0.1,
                        pressure_enabled: true,
                        min_size_fraction: 0.2,
                        smoothing: 0.5,
                        scatter: 0.0,
                        rotation_jitter: 0.0
                    };
                    const id = engine.value.register_brush(JSON.stringify(newBrush));
                    if (id > 0) {
                        // Refresh brushes
                        const brushesJson = engine.value.execute_command(JSON.stringify({ action: 'get_brushes', params: {} }));
                        brushes.value = JSON.parse(brushesJson);
                        selectedBrushId.value = id;
                    }
                }
            };
            img.src = re.target?.result as string;
        };
        reader.readAsDataURL(file);
    };
    input.click();
}

async function removeSelectedBackground() {
    const id = targetImageId.value;
    if (id === -1) return;
    
    const img = imageMap.get(id);
    if (!img) return;

    try {
        // Convert image to blob to ensure compatibility
        const canvas = document.createElement('canvas');
        canvas.width = img.width;
        canvas.height = img.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        ctx.drawImage(img, 0, 0);
        
        const blob = await new Promise<Blob>((resolve) => {
            canvas.toBlob((b) => resolve(b!), 'image/png');
        });

        const resultBlob = await aiService.removeBackground(blob);
        const url = URL.createObjectURL(resultBlob);
        
        // Convert blob back to array buffer for engine
        const resultBuffer = await resultBlob.arrayBuffer();
        const resultBytes = new Uint8Array(resultBuffer);

        const newImg = new Image();
        newImg.onload = () => {
            if (engine.value) {
                engine.value.set_image_object(id, newImg);
                imageMap.set(id, newImg);
                
                // Update raw image data in engine efficiently
                engine.value.set_image_raw(id, resultBytes);

                needsRender.value = true;
                executeCommand({ action: 'update', params: { id, save_undo: true } });
            }
        };
        newImg.src = url;
    } catch (e) {
        console.error("Failed to remove background:", e);
    }
}

async function generativeFill() {
  const p = window.prompt("Enter prompt for generative fill:");
  if (!p) return;
  alert("Generative Fill is currently a placeholder. This would call a Stable Diffusion endpoint with the current selection as a mask.");
}

watch(clipToArtboard, (val) => {
    executeCommand({ action: 'set_clipping', params: { enabled: val } });
});

onMounted(() => {
  if (canvas.value && canvasContainer.value) {
    canvas.value.width = canvasContainer.value.clientWidth;
    canvas.value.height = canvasContainer.value.clientHeight;
    
    try {
        if (typeof VectorEngine === 'undefined') {
            throw new Error("VectorEngine class is not defined. WASM module may have failed to load.");
        }
        engine.value = new VectorEngine();
        updateArtboard();
        
        window.addEventListener('resize', handleResize);
        window.addEventListener('keydown', handleKeydown);
        window.addEventListener('keyup', handleKeyup);

        // Init Viewport
        zoomToFit();

        // Fetch Brushes
        const brushesJson = engine.value.execute_command(JSON.stringify({ action: 'get_brushes', params: {} }));
        brushes.value = JSON.parse(brushesJson);

        renderLoop();
    } catch (err: any) {
        console.error("Engine Initialization Error:", err);
        engineLoadError.value = err.message || "Failed to initialize vector engine.";
    }
  }

  aiService.onStatusUpdate = (status) => {
    aiStatus.value = status;
  };
});

onUnmounted(() => {
    window.removeEventListener('resize', handleResize);
    window.removeEventListener('keydown', handleKeydown);
    window.removeEventListener('keyup', handleKeyup);
    engine.value = null;
});

function handleKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

    if (e.code === 'Space' && !isSpacePressed.value) {
            isSpacePressed.value = true;
            canvas.value!.style.cursor = 'grab';
    }
    
    // Tool Shortcuts
    if (e.key.toLowerCase() === 'v') activeTool.value = 'select';
    if (e.key.toLowerCase() === 'm') activeTool.value = 'rect';
    if (e.key.toLowerCase() === 'b') activeTool.value = 'brush';
    if (e.key.toLowerCase() === 'r') activeTool.value = 'rotate';
    if (e.key.toLowerCase() === 'q') activeTool.value = 'vectorize';
    if (e.key.toLowerCase() === 'c' && hasImage.value) activeTool.value = 'crop';
    if (e.key === 'Backspace' || e.key === 'Delete') deleteSelected();

    // Duplicate Shortcut
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'd') {
        if (selectedIds.value.length > 0) {
                // Send batch duplicate command? Or just duplicate primary?
                // Current engine supports one by one.
                // We'll just duplicate the primary one for now or iterate
                // The engine 'duplicate' command takes an ID and returns a NEW ID.
                // If we duplicate multiple, we probably want to select the new ones.
                // For simplicity, let's duplicate the *last* selected (primary).
                if (selectedObject.value) {
                    executeCommand({ action: 'duplicate', params: { id: selectedObject.value.id } });
                }
            e.preventDefault();
        }
    }

    // Undo/Redo Shortcuts
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') {
        if (e.shiftKey) redo();
        else undo();
        e.preventDefault();
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'y') {
        redo();
        e.preventDefault();
    }

    if (e.key === 'Enter' && bezierState.value.isDrawing) {
        // Remove the "moving" point before finishing
        bezierState.value.points.pop();
        const pts = bezierState.value.points;
        if (pts.length > 0) {
            let minX = Infinity, minY = Infinity;
            pts.forEach(p => {
                minX = Math.min(minX, p.x, p.cin.x, p.cout.x);
                minY = Math.min(minY, p.y, p.cin.y, p.cout.y);
            });

            const newD = getPathString(pts, false, { x: minX, y: minY });
            executeCommand({
                action: 'update',
                params: {
                    id: bezierState.value.currentObjId,
                    path_data: newD,
                    x: minX,
                    y: minY,
                    save_undo: true
                }
            });
        }
        bezierState.value.isDrawing = false;
        bezierState.value.points = [];
        activeTool.value = 'select';
    }
}

function handleKeyup(e: KeyboardEvent) {
    if (e.code === 'Space') {
        isSpacePressed.value = false;
        isPanning.value = false;
        canvas.value!.style.cursor = 'default';
    }
}

function updateArtboard() {
    executeCommand({
        action: 'set_artboard',
        params: {
            width: Number(artboard.value.width),
            height: Number(artboard.value.height),
            background: artboard.value.background
        }
    });
    showDocProps.value = false;
}

function zoomToFit() {
    if (!canvasContainer.value || !engine.value) return;
    const padding = 50;
    const cw = canvasContainer.value.clientWidth;
    const ch = canvasContainer.value.clientHeight;
    const aw = artboard.value.width;
    const ah = artboard.value.height;

    const zoomX = (cw - padding * 2) / aw;
    const zoomY = (ch - padding * 2) / ah;
    const zoom = Math.min(zoomX, zoomY, 1.0);

    const x = (cw - aw * zoom) / 2;
    const y = (ch - ah * zoom) / 2;

    viewport.value = { x, y, zoom };
    engine.value.set_viewport(x, y, zoom);
    needsRender.value = true;
}

function newDocument() {
    if (!engine.value) return;
    // We could add a clear_all command to the engine, or just re-instantiate
    engine.value = new VectorEngine();
    // Re-sync artboard and viewport
    updateArtboard();
    zoomToFit();
    needsRender.value = true;
}

function handleResize() {
  if (canvas.value && canvasContainer.value) {
    canvas.value.width = canvasContainer.value.clientWidth;
    canvas.value.height = canvasContainer.value.clientHeight;
    needsRender.value = true;
  }
}

function reloadApp() {
    window.location.reload();
}

function renderLoop() {
  if (!engine.value || !canvas.value) return;
  
  if (needsRender.value) {
      const ctx = canvas.value.getContext('2d', { willReadFrequently: true });
      if (ctx) {
        engine.value.render(ctx);

        // Draw Eraser Cursor
        if (activeTool.value === 'eraser' && lastMousePos.value && targetImageId.value !== -1) {
            ctx.save();
            ctx.beginPath();
            ctx.arc(lastMousePos.value.x, lastMousePos.value.y, eraserSize.value * viewport.value.zoom, 0, Math.PI * 2);
            ctx.strokeStyle = 'white';
            ctx.lineWidth = 1;
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(lastMousePos.value.x, lastMousePos.value.y, (eraserSize.value * viewport.value.zoom) - 1, 0, Math.PI * 2);
            ctx.strokeStyle = 'rgba(0,0,0,0.5)';
            ctx.setLineDash([2, 2]);
            ctx.stroke();
            ctx.restore();
        }

        // Draw Selection Box
        if (selectionBox.value) {
            ctx.save();
            ctx.strokeStyle = '#4facfe';
            ctx.lineWidth = 1;
            ctx.setLineDash([4, 4]);
            ctx.fillStyle = 'rgba(79, 172, 254, 0.1)';
            
            // The box coords are in screen space
            const { x, y, w, h } = selectionBox.value;
            ctx.fillRect(x, y, w, h);
            ctx.strokeRect(x, y, w, h);
            ctx.restore();
        }

        // Draw Crop Overlay
        if (activeTool.value === 'crop' && cropState.value.isCropping) {
            ctx.save();
            ctx.translate(viewport.value.x, viewport.value.y);
            ctx.scale(viewport.value.zoom, viewport.value.zoom);
            
            const x1 = Math.min(cropState.value.startX, cropState.value.currentX);
            const y1 = Math.min(cropState.value.startY, cropState.value.currentY);
            const w = Math.abs(cropState.value.startX - cropState.value.currentX);
            const h = Math.abs(cropState.value.startY - cropState.value.currentY);
            
            ctx.strokeStyle = '#4facfe';
            ctx.lineWidth = 2 / viewport.value.zoom;
            ctx.strokeRect(x1, y1, w, h);
            
            ctx.fillStyle = 'rgba(79, 172, 254, 0.2)';
            ctx.fillRect(x1, y1, w, h);
            ctx.restore();
        }

        // Draw Bezier Handles
        if ((activeTool.value === 'bezier' || activeTool.value === 'select') && (bezierState.value.isDrawing || bezierState.value.isEditing)) {
            ctx.save();
            ctx.translate(viewport.value.x, viewport.value.y);
            ctx.scale(viewport.value.zoom, viewport.value.zoom);

            const pts = bezierState.value.points;
            pts.forEach((pt, i) => {
                const isLast = i === pts.length - 1;
                
                // Drawing Mode: moving point behavior
                if (bezierState.value.isDrawing && isLast && !isDragging.value) {
                    // Just draw a small dot for the moving point
                    ctx.fillStyle = '#4facfe';
                    ctx.beginPath();
                    ctx.arc(pt.x, pt.y, 3 / viewport.value.zoom, 0, Math.PI * 2);
                    ctx.fill();
                    return;
                }

                // Draw Anchor
                ctx.fillStyle = (bezierState.value.isDrawing && isLast) ? '#ff4f4f' : '#4facfe';
                ctx.strokeStyle = 'white';
                ctx.lineWidth = 1 / viewport.value.zoom;
                ctx.beginPath();
                ctx.rect(pt.x - 4 / viewport.value.zoom, pt.y - 4 / viewport.value.zoom, 8 / viewport.value.zoom, 8 / viewport.value.zoom);
                ctx.fill();
                ctx.stroke();

                // Draw Handles
                ctx.strokeStyle = '#4facfe';
                ctx.lineWidth = 1 / viewport.value.zoom;
                
                // cout
                if (pt.cout.x !== pt.x || pt.cout.y !== pt.y) {
                    ctx.beginPath();
                    ctx.moveTo(pt.x, pt.y);
                    ctx.lineTo(pt.cout.x, pt.cout.y);
                    ctx.stroke();
                    ctx.beginPath();
                    ctx.arc(pt.cout.x, pt.cout.y, 3 / viewport.value.zoom, 0, Math.PI * 2);
                    ctx.stroke();
                    ctx.fillStyle = 'white';
                    ctx.fill();
                }

                // cin
                if (pt.cin.x !== pt.x || pt.cin.y !== pt.y) {
                    ctx.beginPath();
                    ctx.moveTo(pt.x, pt.y);
                    ctx.lineTo(pt.cin.x, pt.cin.y);
                    ctx.stroke();
                    ctx.beginPath();
                    ctx.arc(pt.cin.x, pt.cin.y, 3 / viewport.value.zoom, 0, Math.PI * 2);
                    ctx.stroke();
                    ctx.fillStyle = 'white';
                    ctx.fill();
                }
            });
            ctx.restore();
        }

        // Draw Gradient Controls
        if (activeTool.value === 'gradient' && selectedObject.value && selectedObject.value.fill_gradient) {
            const grad = selectedObject.value.fill_gradient;
            ctx.save();
            ctx.translate(viewport.value.x, viewport.value.y);
            ctx.scale(viewport.value.zoom, viewport.value.zoom);

            // Convert local gradient points to world
            const p1 = localToWorld(selectedObject.value, grad.x1, grad.y1);
            const p2 = localToWorld(selectedObject.value, grad.x2, grad.y2);

            // Draw Line
            ctx.beginPath();
            ctx.moveTo(p1.x, p1.y);
            ctx.lineTo(p2.x, p2.y);
            ctx.strokeStyle = '#000000';
            ctx.lineWidth = 2 / viewport.value.zoom;
            ctx.stroke();
            ctx.strokeStyle = '#ffffff';
            ctx.lineWidth = 1 / viewport.value.zoom;
            ctx.stroke();

            // Draw Endpoints
            [p1, p2].forEach((p) => {
                ctx.beginPath();
                ctx.arc(p.x, p.y, 6 / viewport.value.zoom, 0, Math.PI * 2);
                ctx.fillStyle = '#ffffff';
                ctx.fill();
                ctx.strokeStyle = '#000000';
                ctx.stroke();
            });

            // Draw Stops
            if (grad.stops) {
                grad.stops.forEach((stop: any, idx: number) => {
                    const sx = p1.x + (p2.x - p1.x) * stop.offset;
                    const sy = p1.y + (p2.y - p1.y) * stop.offset;
                    
                    ctx.beginPath();
                    ctx.arc(sx, sy, 5 / viewport.value.zoom, 0, Math.PI * 2);
                    ctx.fillStyle = stop.color;
                    ctx.fill();
                    ctx.strokeStyle = gradientState.value.activeStopIndex === idx ? '#ffff00' : '#ffffff';
                    ctx.lineWidth = 2 / viewport.value.zoom;
                    ctx.stroke();
                    ctx.strokeStyle = '#000000';
                    ctx.lineWidth = 1 / viewport.value.zoom;
                    ctx.stroke();
                });
            }

            ctx.restore();
        }

      }
      needsRender.value = false;
  }
  requestAnimationFrame(renderLoop);
}

function syncState() {
  if (!engine.value) return;
  const json = engine.value.get_objects_json();
  objects.value = JSON.parse(json);
  const selIdsJson = engine.value.get_selected_ids();
  selectedIds.value = JSON.parse(selIdsJson);
  const historyJson = engine.value.execute_command(JSON.stringify({ action: 'get_history', params: {} }));
  history.value = JSON.parse(historyJson);
}

function executeCommand(cmd: any) {
  if (!engine.value) return;
  console.log("Frontend: executeCommand", cmd);
  const result = engine.value.execute_command(JSON.stringify(cmd));
  const parsed = JSON.parse(result);
  if (parsed.error) console.error("Command Error:", parsed.error);
  syncState();
  needsRender.value = true;
  return parsed;
}

function handleWheel(e: WheelEvent) {
  if (!canvas.value || !engine.value) return;
  // e.preventDefault() is handled by @wheel.prevent in template, but good to keep logic clear
  
  const rect = canvas.value.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;

  // Check for Pinch-to-zoom (Ctrl key is usually set by browser)
  // or explicit command+wheel
  if (e.ctrlKey || e.metaKey) {
    const zoomSensitivity = 0.01;
    const delta = -e.deltaY * zoomSensitivity;
    const newZoom = Math.max(0.1, Math.min(50, viewport.value.zoom * Math.exp(delta)));

    const wx = (mx - viewport.value.x) / viewport.value.zoom;
    const wy = (my - viewport.value.y) / viewport.value.zoom;

    viewport.value.zoom = newZoom;
    viewport.value.x = mx - wx * newZoom;
    viewport.value.y = my - wy * newZoom;
  } else {
    // Pan
    viewport.value.x -= e.deltaX;
    viewport.value.y -= e.deltaY;
  }

      engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
      needsRender.value = true;
  }
  
  function hitTestBezierHandles(x: number, y: number): { index: number, type: 'anchor' | 'cin' | 'cout' } | null {
      const pts = bezierState.value.points;
      const hitDist = 8 / viewport.value.zoom;
      
      for (let i = 0; i < pts.length; i++) {
          const pt = pts[i];
          
          // Check Anchor
          if (Math.abs(x - pt.x) < hitDist && Math.abs(y - pt.y) < hitDist) {
              return { index: i, type: 'anchor' };
          }
          
          // Check Cout
          if (Math.abs(x - pt.cout.x) < hitDist && Math.abs(y - pt.cout.y) < hitDist) {
              return { index: i, type: 'cout' };
          }
          
          // Check Cin
          if (Math.abs(x - pt.cin.x) < hitDist && Math.abs(y - pt.cin.y) < hitDist) {
              return { index: i, type: 'cin' };
          }
      }
      return null;
  }
  
  // Pointer Interactions
  function handlePointerDown(e: PointerEvent) {
  console.log("handlePointerDown", activeTool.value);
  if (!canvas.value || !engine.value) return;
  const rect = canvas.value.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  
  dragStart.value = { x, y };
  lastMousePos.value = { x, y };

  if (isSpacePressed.value || activeTool.value === 'hand') {
      isPanning.value = true;
      canvas.value.style.cursor = 'grabbing';
      return;
  }

  isDragging.value = true;
  needsRender.value = true;

  const worldPos = screenToWorld(x, y);

          if (activeTool.value === 'magic_wand') {
              const idsStr = engine.value.select_point(x, y, false, true);
              const ids = JSON.parse(idsStr);
              if (ids.length > 0) {
                  executeCommand({
                      action: 'magic_wand',
                      params: {
                          id: ids[0],
                          x: worldPos.x,
                          y: worldPos.y,
                          tolerance: magicWandTolerance.value
                      }
                  });
              }
              return;
          }
  
          if (activeTool.value === 'clone_stamp') {
              if (e.altKey) {
                  cloneSource.value = { x: worldPos.x, y: worldPos.y };
                  console.log("Clone source set:", cloneSource.value);
                  return;
              }
              if (cloneSource.value && targetImageId.value !== -1) {
                  isDragging.value = true;
                  executeCommand({ action: 'update', params: { id: targetImageId.value, save_undo: true } });
              }
              return;
          }
  
          if (activeTool.value === 'brush') {      if (engine.value) engine.value.hide_selection = true;
      const res = executeCommand({
          action: 'create_brush_stroke',
          params: {
              brush_id: selectedBrushId.value,
              color: brushColor.value,
              points: [{ x: worldPos.x, y: worldPos.y, pressure: e.pressure || 0.5 }],
              save_undo: false
          }
      });
      if (res && res.id) {
          brushState.value.isDrawing = true;
          brushState.value.currentObjId = res.id;
          brushState.value.points = [{ x: worldPos.x, y: worldPos.y, pressure: e.pressure || 0.5 }];
          executeCommand({ action: 'select', params: { id: res.id } });
      }
      return;
  }

  if (activeTool.value === 'pencil') {
      const res = executeCommand({
          action: 'add',
          params: { 
              type: 'Path', 
              x: 0, y: 0, width: 0, height: 0, 
              fill: 'transparent',
              stroke: '#4facfe',
              stroke_width: 2
          }
      });
      if (res && res.id) {
          pencilState.value.isDrawing = true;
          pencilState.value.currentObjId = res.id;
          pencilState.value.points = [{ x: worldPos.x, y: worldPos.y }];
          executeCommand({ action: 'select', params: { id: res.id } });
      }
      return;
  }

  if (activeTool.value === 'eraser') {
      if (targetImageId.value !== -1) {
          // Save undo state before starting erase by sending a dummy update
          executeCommand({ action: 'update', params: { id: targetImageId.value, save_undo: true } });
          
          const modified = engine.value.erase_image(targetImageId.value, worldPos.x, worldPos.y, eraserSize.value);
          if (modified) {
              const pixels = engine.value.get_image_rgba(targetImageId.value);
              const width = engine.value.get_image_width(targetImageId.value);
              const height = engine.value.get_image_height(targetImageId.value);
              if (pixels && width && height) {
                  const tempCanvas = document.createElement('canvas');
                  tempCanvas.width = width;
                  tempCanvas.height = height;
                  const tCtx = tempCanvas.getContext('2d');
                  if (tCtx) {
                      const imageData = new ImageData(new Uint8ClampedArray(pixels), width, height);
                      tCtx.putImageData(imageData, 0, 0);
                      engine.value.set_image_object(targetImageId.value, tempCanvas);
                      needsRender.value = true;
                  }
              }
          }
      }
      return;
  }

  if (activeTool.value === 'gradient') {
      const worldPos = screenToWorld(x, y);
      let obj = selectedObject.value;
      
      // 1. If nothing is selected, try to select something
      if (!obj) {
          const resIds = JSON.parse(engine.value.select_point(x, y, false, false));
          selectedIds.value = resIds;
          syncState();
          obj = selectedObject.value;
      }

      if (!obj) {
          isDragging.value = false;
          gradientState.value.activeStopIndex = -1;
          activeTool.value = 'select';
          return;
      }
      
      // 2. Check if we hit existing controls first (stops or handles)
      if (obj.fill_gradient) {
          const grad = obj.fill_gradient;
          const p1 = localToWorld(obj, grad.x1, grad.y1);
          const p2 = localToWorld(obj, grad.x2, grad.y2);
          const hitDist = 12 / viewport.value.zoom;

          // Hit start handle
          if (Math.hypot(worldPos.x - p1.x, worldPos.y - p1.y) < hitDist) {
              isDragging.value = true;
              gradientState.value = { isDragging: true, dragType: 'start', dragIndex: -1, activeStopIndex: -1 };
              return;
          }
          // Hit end handle
          if (Math.hypot(worldPos.x - p2.x, worldPos.y - p2.y) < hitDist) {
              isDragging.value = true;
              gradientState.value = { isDragging: true, dragType: 'end', dragIndex: -1, activeStopIndex: -1 };
              return;
          }
          // Hit stops
          if (grad.stops) {
              for (let i = 0; i < grad.stops.length; i++) {
                  const stop = grad.stops[i];
                  const sx = p1.x + (p2.x - p1.x) * stop.offset;
                  const sy = p1.y + (p2.y - p1.y) * stop.offset;
                  if (Math.hypot(worldPos.x - sx, worldPos.y - sy) < hitDist) {
                      isDragging.value = true;
                      gradientState.value = { isDragging: true, dragType: 'stop', dragIndex: i, activeStopIndex: i };
                      
                      // Immediate color picker trigger
                      nextTick(() => {
                          const input = document.querySelector('.floating-color-picker input') as HTMLInputElement;
                          input?.click();
                      });
                      return;
                  }
              }
          }
          // Hit line to add stop
          const l2 = (p2.x - p1.x)**2 + (p2.y - p1.y)**2;
          if (l2 > 0) {
              const t = ((worldPos.x - p1.x) * (p2.x - p1.x) + (worldPos.y - p1.y) * (p2.y - p1.y)) / l2;
              if (t >= 0 && t <= 1) {
                  const px = p1.x + t * (p2.x - p1.x);
                  const py = p1.y + t * (p2.y - p1.y);
                  if (Math.hypot(worldPos.x - px, worldPos.y - py) < hitDist) {
                       const newStop = { offset: t, color: '#888888' };
                       const newStops = [...(grad.stops || []), newStop].sort((a: any, b: any) => a.offset - b.offset);
                       const newIdx = newStops.indexOf(newStop);
                       updateGradient('fill', 'stops', newStops);
                       isDragging.value = true;
                       gradientState.value = { isDragging: true, dragType: 'stop', dragIndex: newIdx, activeStopIndex: newIdx };
                       
                       nextTick(() => {
                           const input = document.querySelector('.floating-color-picker input') as HTMLInputElement;
                           input?.click();
                       });
                       return;
                  }
              }
          }
      }

      // 3. If we didn't hit anything specifically, we enter PENDING REDRAW mode
      // It will only start redrawing if the user actually moves the mouse (drag)
      isDragging.value = true;
      gradientState.value = { 
          isDragging: true, 
          dragType: 'redraw_pending' as any, 
          dragIndex: -1, 
          activeStopIndex: -1 
      };
      return;
  }

  if (activeTool.value === 'zoom') {
      const zoomFactor = e.altKey ? 0.5 : 2.0;
      const newZoom = Math.max(0.1, Math.min(50, viewport.value.zoom * zoomFactor));
      
      const wx = (x - viewport.value.x) / viewport.value.zoom;
      const wy = (y - viewport.value.y) / viewport.value.zoom;

      viewport.value.zoom = newZoom;
      viewport.value.x = x - wx * newZoom;
      viewport.value.y = y - wy * newZoom;
      
      engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
      needsRender.value = true;
      return;
  }

  if (activeTool.value === 'bezier') {
      const worldPos = screenToWorld(x, y);

      // Check handle hit first if we have points
      if (bezierState.value.points.length > 0) {
           const hit = hitTestBezierHandles(worldPos.x, worldPos.y);
           if (hit) {
               bezierState.value.dragIndex = hit.index;
               bezierState.value.dragType = hit.type;
               isDragging.value = true;
               return;
           }
      }

      if (!bezierState.value.isDrawing && !bezierState.value.isEditing) {
          // Check if we hit an EXISTING path object to start editing
          const idsStr = engine.value.select_point(x, y, false, false);
          const ids = JSON.parse(idsStr);
          if (ids.length > 0) {
               const obj = objects.value.find(o => o.id === ids[0]);
               if (obj && obj.shape_type === 'Path') {
                   bezierState.value.isEditing = true;
                   bezierState.value.currentObjId = obj.id;
                   bezierState.value.points = parsePathData(obj.path_data, obj.x, obj.y);
                   executeCommand({ action: 'select', params: { id: obj.id } });
                   return;
               }
          }
          
          // Start new path
           const res = executeCommand({
                action: 'add',
                params: { 
                    type: 'Path', 
                    x: 0, y: 0, width: 0, height: 0, 
                    fill: 'transparent',
                    stroke: '#4facfe',
                    stroke_width: 2
                }
            });
            
            if (res && res.id) {
                bezierState.value.isDrawing = true;
                bezierState.value.currentObjId = res.id;
                bezierState.value.points = [
                    { x: worldPos.x, y: worldPos.y, cin: {x: worldPos.x, y: worldPos.y}, cout: {x: worldPos.x, y: worldPos.y} }
                ];
                bezierState.value.mousePoint = { x: worldPos.x, y: worldPos.y };
                executeCommand({ action: 'select', params: { id: res.id } });
            }
      } else if (bezierState.value.isDrawing) {
          // Check for Snap-Close
          let isClose = false;
          const startPt = bezierState.value.points[0];
          const dist = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
          if (snapMode.value && dist < 15 / viewport.value.zoom) {
              isClose = true;
          }

          if (isClose) {
                bezierState.value.isClosing = true;
                bezierState.value.mousePoint = null; 
          } else {
              bezierState.value.points.push({ 
                  x: worldPos.x, y: worldPos.y, 
                  cin: {x: worldPos.x, y: worldPos.y}, 
                  cout: {x: worldPos.x, y: worldPos.y} 
              });
              
              const pts = bezierState.value.points;
              const d = getPathString(pts, false, null, { x: 0, y: 0 });
              executeCommand({
                    action: 'update',
                    params: { 
                        id: bezierState.value.currentObjId, 
                        path_data: d,
                        x: 0,
                        y: 0 
                    }
                });
          }
      } else if (bezierState.value.isEditing) {
          // If in edit mode and didn't hit a handle, we exit edit mode
          bezierState.value.isEditing = false;
          bezierState.value.points = [];
          bezierState.value.currentObjId = -1;
          activeTool.value = 'select';
      }
      return;
  }

  if (activeTool.value === 'select') {
    // Check Bezier handles first if editing
    if (bezierState.value.isEditing) {
        const worldPos = screenToWorld(x, y);
        const hit = hitTestBezierHandles(worldPos.x, worldPos.y);
        if (hit) {
            bezierState.value.dragIndex = hit.index;
            bezierState.value.dragType = hit.type;
            isDragging.value = true;
            return;
        }
    }

    // Check handles first
    const handleHitStr = engine.value?.hit_test_handles(x, y);
    const handleHit = handleHitStr ? JSON.parse(handleHitStr) : null;
    
    if (handleHit) {
        activeHandle.value = { id: handleHit[0], type: handleHit[1] };
        
        // Prepare initial state for transformation
        initialObjectsState.value.clear();
        objects.value
            .filter(o => selectedIds.value.includes(o.id))
            .forEach(o => {
                initialObjectsState.value.set(o.id, { x: o.x, y: o.y, width: o.width, height: o.height, rotation: o.rotation });
            });
        return;
    }

    // 1. Perform selection
    const idsStr = engine.value.select_point(x, y, e.shiftKey || e.metaKey, false);
    selectedIds.value = JSON.parse(idsStr);

    // 2. Check if we are dragging a selected object
    // Simple hit test against selected objects
    const isOverSelected = objects.value
        .filter(o => selectedIds.value.includes(o.id))
        .some(o => {
            return worldPos.x >= o.x && worldPos.x <= o.x + o.width &&
                   worldPos.y >= o.y && worldPos.y <= o.y + o.height;
        });

    if (isOverSelected) {
        // Prepare for dragging
        initialObjectsState.value.clear();
        objects.value
            .filter(o => selectedIds.value.includes(o.id))
            .forEach(o => {
                initialObjectsState.value.set(o.id, { x: o.x, y: o.y, width: o.width, height: o.height, rotation: o.rotation });
            });
    } else {
        // Box Select
        isDragging.value = false;
        selectionBox.value = { x, y, w: 0, h: 0 };
    }

  } else if (activeTool.value === 'crop') {
    cropState.value = {
        isCropping: true,
        startX: worldPos.x,
        startY: worldPos.y,
        currentX: worldPos.x,
        currentY: worldPos.y,
    };
  } else if (['rect', 'circle', 'star', 'poly'].includes(activeTool.value)) {
    const type = activeTool.value === 'rect' ? 'Rectangle' : 
                 activeTool.value === 'circle' ? 'Circle' :
                 activeTool.value === 'star' ? 'Star' : 'Polygon';
    const res = executeCommand({
        action: 'add',
        params: { type, x: worldPos.x, y: worldPos.y, width: 1, height: 1, fill: '#4facfe' }
    });
    if (res && res.id) {
        executeCommand({ action: 'select', params: { id: res.id } });
        
        // Prepare for dragging/resizing immediately
        initialObjectsState.value.clear();
        initialObjectsState.value.set(res.id, { x: worldPos.x, y: worldPos.y, width: 1, height: 1, rotation: 0 });
        isDragging.value = true;
    }
  } else if (activeTool.value === 'text') {
    const res = executeCommand({
        action: 'add',
        params: { type: 'Text', x: worldPos.x, y: worldPos.y, width: 200, height: 40, fill: '#000000' }
    });
    if (res.id) {
        executeCommand({ action: 'select', params: { id: res.id } });
    }
  } else if (activeTool.value === 'eyedropper') {
      const ctx = canvas.value.getContext('2d');
      if (ctx) {
          // We need to sample from the ACTUAL canvas pixels
          const pixel = ctx.getImageData(x, y, 1, 1).data;
          const hex = "#" + ((1 << 24) + (pixel[0] << 16) + (pixel[1] << 8) + pixel[2]).toString(16).slice(1);
          if (selectedIds.value.length > 0) {
              updateSelected('fill', hex);
          }
      }
  } else if (activeTool.value === 'vectorize') {
      console.log("Vectorize tool: click at", x, y);
      const idsStr = engine.value.select_point(x, y, false, true);
      const ids = JSON.parse(idsStr);
      console.log("Vectorize tool: hit ids", ids);
      if (ids.length > 0) {
          selectedIds.value = ids;
          syncState();
      }
      // Always try to vectorize what we have (or the background image as fallback)
      nextTick(() => {
          vectorizeImage();
      });
      return;
  }
}

function handlePointerMove(e: PointerEvent) {
  if (!canvas.value || !engine.value) return;
  const rect = canvas.value.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;

  if (activeTool.value === 'eraser') {
      lastMousePos.value = { x, y };
      needsRender.value = true;
  }

  if (isPanning.value || activeTool.value === 'hand') {
      canvas.value.style.cursor = isPanning.value ? 'grabbing' : 'grab';
  }

  if (isPanning.value) {
      const dx = x - lastMousePos.value.x;
      const dy = y - lastMousePos.value.y;
      
      viewport.value.x += dx;
      viewport.value.y += dy;
      
      engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
      needsRender.value = true;
      lastMousePos.value = { x, y };
      return;
  }

  if (selectionBox.value) {
      selectionBox.value.w = x - selectionBox.value.x;
      selectionBox.value.h = y - selectionBox.value.y;
      needsRender.value = true;
      lastMousePos.value = { x, y };
      return;
  }

  const worldPos = screenToWorld(x, y);

  // Set Cursor
  if (isPanning.value || activeTool.value === 'hand') {
      canvas.value.style.cursor = isPanning.value ? 'grabbing' : 'grab';
  } else if (activeTool.value === 'pencil' || activeTool.value === 'brush' || activeTool.value === 'bezier' || activeTool.value === 'vectorize' || activeTool.value === 'eraser') {
      canvas.value.style.cursor = 'crosshair';
      if (activeTool.value === 'bezier' && bezierState.value.isDrawing && !isDragging.value && snapMode.value && bezierState.value.points.length > 2) {
          const startPt = bezierState.value.points[0];
          const dist = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
          if (dist < 15 / viewport.value.zoom) {
              canvas.value.style.cursor = 'pointer';
          }
      }
  } else if (activeTool.value === 'zoom') {
      canvas.value.style.cursor = e.altKey ? 'zoom-out' : 'zoom-in';
  } else {
      canvas.value.style.cursor = 'default';
  }

  if (activeTool.value === 'brush') {
      if (brushState.value.isDrawing) {
          const lastPt = brushState.value.points[brushState.value.points.length - 1];
          const dist = Math.sqrt((worldPos.x - lastPt.x)**2 + (worldPos.y - lastPt.y)**2);
          
          if (dist > 2 / viewport.value.zoom) {
              brushState.value.points.push({ x: worldPos.x, y: worldPos.y, pressure: e.pressure || 0.5 });
              
              if (brushState.value.currentObjId === -1) {
                  // Create initial stroke
                  const res = executeCommand({
                      action: 'create_brush_stroke',
                      params: {
                          brush_id: selectedBrushId.value,
                          color: brushColor.value,
                          points: brushState.value.points,
                          save_undo: false
                      }
                  });
                  if (res && res.id) brushState.value.currentObjId = res.id;
              } else {
                  // Update existing stroke
                  executeCommand({
                      action: 'update_brush_stroke',
                      params: {
                          id: brushState.value.currentObjId,
                          brush_id: selectedBrushId.value,
                          points: brushState.value.points
                      }
                  });
                  needsRender.value = true;
              }
          }
          return;
      }
  }

  if (activeTool.value === 'pencil') {
      if (pencilState.value.isDrawing) {
          const lastPt = pencilState.value.points[pencilState.value.points.length - 1];
          const dist = Math.sqrt((worldPos.x - lastPt.x)**2 + (worldPos.y - lastPt.y)**2);
          
          if (dist > 3 / viewport.value.zoom) {
              pencilState.value.points.push({ x: worldPos.x, y: worldPos.y });
              const d = getPencilPathString(pencilState.value.points, { x: 0, y: 0 });
              executeCommand({
                  action: 'update',
                  params: { 
                      id: pencilState.value.currentObjId, 
                      path_data: d,
                      x: 0, y: 0, width: 0, height: 0
                  }
              });
          }
          return;
      }
  }

  if (activeTool.value === 'clone_stamp' && isDragging.value && cloneSource.value && targetImageId.value !== -1) {
      const worldPos = screenToWorld(x, y);
      const dx = worldPos.x - dragStart.value.x; // Wait, we need offset from start
      // Actually, we need to track the current source point relative to where we started dragging
      const currentSrcX = cloneSource.value.x + (worldPos.x - screenToWorld(dragStart.value.x, dragStart.value.y).x);
      const currentSrcY = cloneSource.value.y + (worldPos.y - screenToWorld(dragStart.value.x, dragStart.value.y).y);

      const modified = engine.value.clone_stamp(
          targetImageId.value, 
          currentSrcX, currentSrcY, 
          worldPos.x, worldPos.y, 
          cloneSize.value
      );
      
      if (modified) {
          updateImageFromEngine(targetImageId.value);
      }
      return;
  }

  if (activeTool.value === 'eraser' && isDragging.value) {
      if (targetImageId.value !== -1) {
          const worldPos = screenToWorld(x, y);
          const modified = engine.value.erase_image(targetImageId.value, worldPos.x, worldPos.y, eraserSize.value);
          if (modified) {
              updateImageFromEngine(targetImageId.value);
          }
      }
      return;
  }

  if (activeTool.value === 'gradient') {
      if (gradientState.value.isDragging && selectedObject.value) {
          const obj = selectedObject.value;
          const grad = obj.fill_gradient;
          const type = gradientState.value.dragType;
          
          if (type === 'redraw_pending' as any) {
              const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
              const dist = Math.hypot(worldPos.x - startWorld.x, worldPos.y - startWorld.y);
              if (dist > 5 / viewport.value.zoom) {
                  // Transition to end-drag mode
                  gradientState.value.dragType = 'end';
                  const localStart = worldToLocal(obj, startWorld.x, startWorld.y);
                  const defaultStops = obj.fill_gradient?.stops || [
                      { offset: 0, color: '#ffffff' },
                      { offset: 1, color: '#000000' }
                  ];
                  updateSelected('fill_gradient', {
                      is_radial: false,
                      x1: localStart.x, y1: localStart.y,
                      x2: localStart.x, y2: localStart.y,
                      r1: 0, r2: 50,
                      stops: defaultStops
                  }, false);
              }
              return;
          }

          if (!grad) return;

          if (type === 'start') {
              const localPt = worldToLocal(obj, worldPos.x, worldPos.y);
              updateSelected('fill_gradient', { ...grad, x1: localPt.x, y1: localPt.y }, false);
          } else if (type === 'end') {
              const localPt = worldToLocal(obj, worldPos.x, worldPos.y);
              updateSelected('fill_gradient', { ...grad, x2: localPt.x, y2: localPt.y }, false);
          } else if (type === 'stop') {
              const p1 = localToWorld(obj, grad.x1, grad.y1);
              const p2 = localToWorld(obj, grad.x2, grad.y2);
              const l2 = (p2.x - p1.x)**2 + (p2.y - p1.y)**2;
              if (l2 > 0) {
                  let t = ((worldPos.x - p1.x) * (p2.x - p1.x) + (worldPos.y - p1.y) * (p2.y - p1.y)) / l2;
                  t = Math.max(0, Math.min(1, t));
                  updateGradientStop('fill', gradientState.value.dragIndex, 'offset', t, false);
              }
          }
          return;
      }
  }

  if (!isDragging.value && !cropState.value.isCropping && !activeHandle.value) return;
  
  if (activeHandle.value && selectedObject.value) {
      const initial = initialObjectsState.value.get(activeHandle.value.id);
      if (!initial) return;

      const worldPos = screenToWorld(x, y);
      
      if (activeHandle.value.type === 'Rotate') {
          const centerX = initial.x + initial.width / 2;
          const centerY = initial.y + initial.height / 2;
          const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
          const startAngle = Math.atan2(startWorld.y - centerY, startWorld.x - centerX);
          const currentAngle = Math.atan2(worldPos.y - centerY, worldPos.x - centerX);
          updateSelected('rotation', initial.rotation + (currentAngle - startAngle), false);
      } else {
          // Resizing logic: Keep opposite side fixed
          const type = activeHandle.value.type;
          
          // 1. Get mouse position in object's local space (relative to its INITIAL top-left)
          const dx = worldPos.x - initial.x;
          const dy = worldPos.y - initial.y;
          const cos_r = Math.cos(-initial.rotation);
          const sin_r = Math.sin(-initial.rotation);
          
          // Local mouse position relative to initial top-left
          let lx = dx * cos_r - dy * sin_r;
          let ly = dx * sin_r + dy * cos_r;

          let newWidth = initial.width;
          let newHeight = initial.height;
          let localPivotX = 0;
          let localPivotY = 0;

          if (type.includes('Right')) {
              newWidth = lx;
              localPivotX = 0;
          } else if (type.includes('Left')) {
              newWidth = initial.width - lx;
              localPivotX = initial.width;
          }

          if (type.includes('Bottom')) {
              newHeight = ly;
              localPivotY = 0;
          } else if (type.includes('Top')) {
              newHeight = initial.height - ly;
              localPivotY = initial.height;
          }

          const isCorner = ['TopLeft', 'TopRight', 'BottomLeft', 'BottomRight'].includes(type);
          if (isCorner) {
              const ratio = initial.width / initial.height;
              if (newWidth / ratio > newHeight) {
                  newHeight = newWidth / ratio;
              } else {
                  newWidth = newHeight * ratio;
              }
              
              // Re-adjust lx/ly based on aspect ratio constraint if we moved Left or Top
              if (type.includes('Left')) lx = initial.width - newWidth;
              if (type.includes('Top')) ly = initial.height - newHeight;
          }

          newWidth = Math.max(1, newWidth);
          newHeight = Math.max(1, newHeight);

          // 2. Calculate the world position of the pivot (the point that stays fixed)
          const cos_ir = Math.cos(initial.rotation);
          const sin_ir = Math.sin(initial.rotation);
          const pivotWorldX = initial.x + (localPivotX * cos_ir - localPivotY * sin_ir);
          const pivotWorldY = initial.y + (localPivotX * sin_ir + localPivotY * cos_ir);

          // 3. The new top-left is pivotWorld minus the new local pivot position rotated
          // New local pivot depends on which handle we are dragging
          let newLocalPivotX = 0;
          let newLocalPivotY = 0;
          if (type.includes('Left')) newLocalPivotX = newWidth;
          if (type.includes('Top')) newLocalPivotY = newHeight;

          const newX = pivotWorldX - (newLocalPivotX * cos_ir - newLocalPivotY * sin_ir);
          const newY = pivotWorldY - (newLocalPivotX * sin_ir + newLocalPivotY * cos_ir);

          executeCommand({
              action: 'update',
              params: {
                  id: activeHandle.value.id,
                  x: newX,
                  y: newY,
                  width: newWidth,
                  height: newHeight,
                  save_undo: false
              }
          });
      }
      needsRender.value = true;
      return;
  }
  
  if (activeTool.value === 'crop' && cropState.value.isCropping) {
    cropState.value.currentX = worldPos.x;
    cropState.value.currentY = worldPos.y;
    needsRender.value = true;
    return;
  }

  if ((activeTool.value === 'bezier' || activeTool.value === 'select') && bezierState.value.isEditing && isDragging.value && bezierState.value.dragIndex !== -1) {
      const pts = bezierState.value.points;
      const idx = bezierState.value.dragIndex;
      const type = bezierState.value.dragType;
      const pt = pts[idx];
      
      if (type === 'anchor') {
          const dx = worldPos.x - pt.x;
          const dy = worldPos.y - pt.y;
          pt.x = worldPos.x;
          pt.y = worldPos.y;
          pt.cin.x += dx;
          pt.cin.y += dy;
          pt.cout.x += dx;
          pt.cout.y += dy;
      } else if (type === 'cin') {
          pt.cin = { x: worldPos.x, y: worldPos.y };
      } else if (type === 'cout') {
          pt.cout = { x: worldPos.x, y: worldPos.y };
      }
      
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      pts.forEach(p => {
          minX = Math.min(minX, p.x, p.cin.x, p.cout.x);
          minY = Math.min(minY, p.y, p.cin.y, p.cout.y);
          maxX = Math.max(maxX, p.x, p.cin.x, p.cout.x);
          maxY = Math.max(maxY, p.y, p.cin.y, p.cout.y);
      });
      
      const d = getPathString(pts, bezierState.value.isClosing, null, { x: minX, y: minY });
      
      executeCommand({
          action: 'update',
          params: {
              id: bezierState.value.currentObjId,
              path_data: d,
              x: minX,
              y: minY,
              width: Math.max(1, maxX - minX),
              height: Math.max(1, maxY - minY),
              save_undo: false
          }
      });
      return;
  }

  if (activeTool.value === 'bezier') {
      const pts = bezierState.value.points;
      
      // EDIT MODE (already handled above for both select/bezier)
      if (bezierState.value.isEditing && isDragging.value && bezierState.value.dragIndex !== -1) {
          return;
      }

      if (bezierState.value.isDrawing) {
        // 1. Update mouse preview point
        let targetX = worldPos.x;
        let targetY = worldPos.y;
        bezierState.value.isSnapped = false;

        if (snapMode.value && pts.length > 0) {
            const startPt = pts[0];
            const dStart = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
            if (dStart < 15 / viewport.value.zoom) {
                targetX = startPt.x;
                targetY = startPt.y;
                bezierState.value.isSnapped = true;
            }
        }
        bezierState.value.mousePoint = { x: targetX, y: targetY };

        // 2. If dragging, adjust handles
        if (isDragging.value && pts.length > 0) {
            const pt = bezierState.value.isClosing ? pts[0] : pts[pts.length - 1];
            const dist = Math.sqrt((worldPos.x - pt.x)**2 + (worldPos.y - pt.y)**2);
            if (dist > 2 / viewport.value.zoom) {
                pt.cout = { x: worldPos.x, y: worldPos.y };
                pt.cin = { 
                    x: pt.x - (worldPos.x - pt.x),
                    y: pt.y - (worldPos.y - pt.y) 
                };
            }
        }

        // 3. Update the path data in world space (0,0)
        const d = getPathString(pts, bezierState.value.isClosing, bezierState.value.mousePoint, { x: 0, y: 0 });
        executeCommand({
                action: 'update',
                params: { 
                    id: bezierState.value.currentObjId, 
                    path_data: d,
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0
                }
            });
        return;
      }
  }

  if (activeTool.value === 'rotate' && isDragging.value && selectedObject.value) {
      const centerX = selectedObject.value.x + selectedObject.value.width / 2;
      const centerY = selectedObject.value.y + selectedObject.value.height / 2;
      
      const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
      const startAngle = Math.atan2(startWorld.y - centerY, startWorld.x - centerX);
      const currentAngle = Math.atan2(worldPos.y - centerY, worldPos.x - centerX);
      
      const deltaAngle = currentAngle - startAngle;
      const initial = initialObjectsState.value.get(selectedObject.value.id);
      if (initial) {
          updateSelected('rotation', initial.rotation + deltaAngle, false);
      }
      return;
  }

  if (activeTool.value === 'select' && isDragging.value && selectedIds.value.length > 0) {
    const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
    const dx = worldPos.x - startWorld.x;
    const dy = worldPos.y - startWorld.y;

    selectedIds.value.forEach(id => {
        const init = initialObjectsState.value.get(id);
        if (init) {
             executeCommand({
                action: 'update',
                params: { 
                    id, 
                    x: init.x + dx, 
                    y: init.y + dy,
                    save_undo: false
                }
            });
        }
    });

    if (bezierState.value.isEditing && selectedIds.value.length === 1) {
        const obj = objects.value.find(o => o.id === selectedIds.value[0]);
        if (obj) {
            bezierState.value.points = parsePathData(obj.path_data, obj.x, obj.y);
        }
    }
  } else if (['rect', 'circle', 'star', 'poly'].includes(activeTool.value) && selectedObject.value) {
    const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
    const width = Math.max(1, worldPos.x - startWorld.x);
    const height = Math.max(1, worldPos.y - startWorld.y);
    executeCommand({
        action: 'update',
        params: { id: selectedObject.value.id, width, height }
    });
  }

  lastMousePos.value = { x, y };
}

function handlePointerUp(e: PointerEvent) {
  if (isPanning.value) {
      isPanning.value = false;
      if (canvas.value) canvas.value.style.cursor = isSpacePressed.value ? 'grab' : 'default';
  }

  if (selectionBox.value) {
      try {
          const { x, y, w, h } = selectionBox.value;
          if (Math.abs(w) > 2 && Math.abs(h) > 2) {
              const idsStr = engine.value?.select_rect(x, y, w, h, e.shiftKey || e.metaKey, false);
              if (idsStr) selectedIds.value = JSON.parse(idsStr);
          }
      } finally {
          selectionBox.value = null;
          needsRender.value = true;
      }
      // We don't return here because we might need to reset tools or other cleanup
  }

  if (activeTool.value === 'brush' && brushState.value.isDrawing) {
      if (engine.value) engine.value.hide_selection = false;
      if (brushState.value.points.length > 1 && brushState.value.currentObjId !== -1) {
          // Finalize the stroke with an undo point
          // We can just update it one last time with save_undo: true
          // Or just save_state manually. 
          // The simplest is to create it again with save_undo: true and delete the temp one.
          // Wait, actually we can just call update with save_undo: true.
          executeCommand({
              action: 'update',
              params: {
                  id: brushState.value.currentObjId,
                  save_undo: true
              }
          });
      } else if (brushState.value.currentObjId !== -1) {
          // Delete if too short
          executeCommand({ action: 'delete', params: { id: brushState.value.currentObjId, save_undo: false } });
      }
      brushState.value.isDrawing = false;
      brushState.value.points = [];
      brushState.value.currentObjId = -1;
      syncState();
      needsRender.value = true;
      return;
  }

  if (activeTool.value === 'crop' && cropState.value.isCropping) {
      const x1 = Math.min(cropState.value.startX, cropState.value.currentX);
      const y1 = Math.min(cropState.value.startY, cropState.value.currentY);
      const w = Math.abs(cropState.value.startX - cropState.value.currentX);
      const h = Math.abs(cropState.value.startY - cropState.value.currentY);

      if (w > 5 && h > 5) {
          // Identify background image if any
          const bgImage = objects.value.find(o => o.shape_type === 'Image' && o.name === 'Background Image');

          if (bgImage) {
              const scaleX = bgImage.sw / bgImage.width;
              const scaleY = bgImage.sh / bgImage.height;
              
              const newSx = bgImage.sx + (x1 - bgImage.x) * scaleX;
              const newSy = bgImage.sy + (y1 - bgImage.y) * scaleY;
              const newSw = w * scaleX;
              const newSh = h * scaleY;

              executeCommand({
                  action: 'update',
                  params: {
                      id: bgImage.id,
                      x: 0,
                      y: 0,
                      width: w,
                      height: h,
                      sx: newSx,
                      sy: newSy,
                      sw: newSw,
                      sh: newSh,
                      save_undo: true
                  }
              });

              // Shift other objects
              objects.value.forEach(obj => {
                  if (obj.id !== bgImage.id) {
                      executeCommand({
                          action: 'update',
                          params: {
                              id: obj.id,
                              x: obj.x - x1,
                              y: obj.y - y1
                          }
                      });
                  }
              });
          } else {
              // Standard behavior: shift all objects
              let first = true;
              objects.value.forEach(obj => {
                  executeCommand({
                      action: 'update',
                      params: {
                          id: obj.id,
                          x: obj.x - x1,
                          y: obj.y - y1,
                          save_undo: first
                      }
                  });
                  first = false;
              });
          }

          // 2. Update artboard
          artboard.value.width = w;
          artboard.value.height = h;
          updateArtboard();

          // 3. Adjust viewport to keep things centered
          viewport.value.x += x1 * viewport.value.zoom;
          viewport.value.y += y1 * viewport.value.zoom;
          engine.value?.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
      }
      
      cropState.value.isCropping = false;
      activeTool.value = 'select';
  }

  if (activeHandle.value) {
      executeCommand({
          action: 'update',
          params: { id: activeHandle.value.id, save_undo: true }
      });
      activeHandle.value = null;
  }

  if (activeTool.value === 'select' && isDragging.value && selectedIds.value.length > 0) {
      // Create undo point at the end of dragging
      // We trigger a save_undo by updating the first selected object with no changes (or just save_undo flag)
      executeCommand({
          action: 'update',
          params: { id: selectedIds.value[0], save_undo: true }
      });
  }

  if ((activeTool.value === 'bezier' || activeTool.value === 'select') && bezierState.value.isEditing) {
       if (bezierState.value.dragIndex !== -1) {
           executeCommand({
                action: 'update',
                params: {
                    id: bezierState.value.currentObjId,
                    save_undo: true
                }
           });
       }
       bezierState.value.dragIndex = -1;
       bezierState.value.dragType = null;
  }

  if (activeTool.value === 'bezier' && bezierState.value.isDrawing && bezierState.value.isClosing) {
      // Finalize the closed path
      const pts = bezierState.value.points;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      pts.forEach(p => {
          minX = Math.min(minX, p.x, p.cin.x, p.cout.x);
          minY = Math.min(minY, p.y, p.cin.y, p.cout.y);
          maxX = Math.max(maxX, p.x, p.cin.x, p.cout.x);
          maxY = Math.max(maxY, p.y, p.cin.y, p.cout.y);
      });

      const newD = getPathString(pts, true, null, { x: minX, y: minY });
      executeCommand({
          action: 'update',
          params: {
              id: bezierState.value.currentObjId,
              path_data: newD,
              x: minX,
              y: minY,
              width: Math.max(1, maxX - minX),
              height: Math.max(1, maxY - minY),
              save_undo: true
          }
      });
      
      bezierState.value.isDrawing = false;
      bezierState.value.isClosing = false;
      bezierState.value.points = [];
      bezierState.value.mousePoint = null;
      activeTool.value = 'select';
  }

  if (activeTool.value === 'pencil' && pencilState.value.isDrawing) {
      const pts = pencilState.value.points;
      if (pts.length > 1) {
          let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
          pts.forEach(p => {
              minX = Math.min(minX, p.x);
              minY = Math.min(minY, p.y);
              maxX = Math.max(maxX, p.x);
              maxY = Math.max(maxY, p.y);
          });

          const d = getPencilPathString(pts, { x: minX, y: minY });
          executeCommand({
              action: 'update',
              params: {
                  id: pencilState.value.currentObjId,
                  path_data: d,
                  x: minX,
                  y: minY,
                  width: Math.max(1, maxX - minX),
                  height: Math.max(1, maxY - minY),
                  save_undo: true
              }
          });
      } else {
          // Delete if only one point
          executeCommand({ action: 'delete', params: { id: pencilState.value.currentObjId } });
      }
      pencilState.value.isDrawing = false;
      pencilState.value.points = [];
      activeTool.value = 'select';
  }

  if (activeTool.value === 'gradient') {
      if (gradientState.value.isDragging) {
           if (gradientState.value.dragType === 'stop' && selectedObject.value) {
               const grad = { ...selectedObject.value.fill_gradient };
               grad.stops.sort((a: any, b: any) => a.offset - b.offset);
               updateSelected('fill_gradient', grad, true);
           } else if (selectedIds.value.length > 0) {
               executeCommand({
                    action: 'update',
                    params: {
                        id: selectedIds.value[0],
                        save_undo: true
                    }
               });
           }
      }
      gradientState.value.isDragging = false;
      gradientState.value.dragIndex = -1;
      gradientState.value.dragType = null;
  }

  isDragging.value = false;
  initialObjectsState.value.clear();
  
  const whitelist = ['select', 'bezier', 'pencil', 'brush', 'eraser', 'hand', 'zoom', 'rotate', 'gradient', 'vectorize', 'rect', 'circle', 'star', 'poly', 'text', 'eyedropper', 'crop', 'image'];
  if (!whitelist.includes(activeTool.value)) {
      activeTool.value = 'select';
  }
}

// AI Integration
async function sendMessage() {
  if (!chatInput.value.trim() || !engine.value) return;
  
  const userText = chatInput.value;
  messages.value.push({ role: 'user', content: userText });
  chatInput.value = '';

  const objectsJson = engine.value.get_objects_json();
  const thinkingId = messages.value.push({ role: 'ai', content: "Thinking..." }) - 1;

  try {
    const aiCommands = await aiService.processPrompt(userText, objectsJson);
    messages.value.splice(thinkingId, 1);

    for (const cmd of aiCommands) {
      // Execute AI command directly
      executeCommand({ action: cmd.action, params: cmd.params });
    }

    if (aiCommands.length > 0) {
      messages.value.push({ role: 'ai', content: `Executed ${aiCommands.length} AI commands.` });
    } else {
      messages.value.push({ role: 'ai', content: "I couldn't understand that command." });
    }
  } catch (e) {
    messages.value.splice(thinkingId, 1);
    messages.value.push({ role: 'ai', content: "Error communicating with AI." });
  }
}

function initGradient(prop: 'fill' | 'stroke') {
    if (selectedIds.value.length === 0) return;
    const defaultGradient = {
        is_radial: false,
        x1: 0, y1: 0, x2: 100, y2: 0,
        r1: 0, r2: 50,
        stops: [
            { offset: 0, color: '#ffffff' },
            { offset: 1, color: '#000000' }
        ]
    };
    updateSelected(prop + '_gradient', defaultGradient);
}

function updateGradient(prop: 'fill' | 'stroke', key: string, value: any, saveUndo: boolean = true) {
    if (!selectedObject.value) return;
    const grad = { ...selectedObject.value[prop + '_gradient'] };
    grad[key] = value;
    updateSelected(prop + '_gradient', grad, saveUndo);
}

function updateGradientStop(prop: 'fill' | 'stroke', idx: number, key: string, value: any, saveUndo: boolean = true) {
    if (!selectedObject.value) return;
    const grad = { ...selectedObject.value[prop + '_gradient'] };
    const stops = [...grad.stops];
    stops[idx] = { ...stops[idx], [key]: value };
    grad.stops = stops;
    updateSelected(prop + '_gradient', grad, saveUndo);
}

function addGradientStop(prop: 'fill' | 'stroke') {
    if (!selectedObject.value) return;
    const grad = { ...selectedObject.value[prop + '_gradient'] };
    grad.stops.push({ offset: 0.5, color: '#888888' });
    grad.stops.sort((a: any, b: any) => a.offset - b.offset);
    updateSelected(prop + '_gradient', grad);
}

function removeGradientStop(prop: 'fill' | 'stroke', idx: number) {
    if (!selectedObject.value) return;
    const grad = { ...selectedObject.value[prop + '_gradient'] };
    if (grad.stops.length <= 2) return; // Min 2 stops
    grad.stops.splice(idx, 1);
    updateSelected(prop + '_gradient', grad);
}

function addEffect(type: string) {

  if (!selectedObject.value) return;

  const effect = {

    effect_type: type,

    enabled: true,

    color: '#000000',

    opacity: 0.5,

    blur: 10,

    x: 5,

    y: 5,

    size: 0,

    spread: 0,

    blend_mode: 'normal'

  };

  const newStyle = {

    effects: [...selectedObject.value.layer_style.effects, effect]

  };

  updateSelected('layer_style', newStyle);

}



function removeEffect(index: number) {

  if (!selectedObject.value) return;

  const newEffects = [...selectedObject.value.layer_style.effects];

  newEffects.splice(index, 1);

  updateSelected('layer_style', { effects: newEffects });

}



function updateLayerStyle() {

  if (!selectedObject.value) return;

  updateSelected('layer_style', selectedObject.value.layer_style);

}



function jumpToHistory(index: number) {

  if (!engine.value) return;

  // This is a simplified "jump" by just calling undo/redo until we reach the target.

  // In a real app, you'd store full states or use a more efficient delta system.

  const currentHistoryLen = history.value.length;

  const targetLen = index + 1;

  

  if (targetLen < currentHistoryLen) {

    for (let i = 0; i < currentHistoryLen - targetLen; i++) {

      engine.value.undo();

    }

  }

  syncState();

  needsRender.value = true;

}



function updateSelected(key: string, value: any, saveUndo: boolean = true) {

  if (selectedIds.value.length === 0) return;

  executeCommand({

    action: 'update',

    params: {

      ids: selectedIds.value,

      [key]: value,

      save_undo: saveUndo

    }

  });

}



function deleteSelected() {
    if (selectedIds.value.length === 0) return;
    executeCommand({ action: 'delete', params: { ids: selectedIds.value } });
}

function undo() {
    if (engine.value?.undo()) {
        needsRender.value = true;
    }
}

function redo() {
    if (engine.value?.redo()) {
        needsRender.value = true;
    }
}

function handleOpenImage(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file || !engine.value) return;

  const reader = new FileReader();
  reader.onload = (e) => {
    const result = e.target?.result as ArrayBuffer;
    const bytes = new Uint8Array(result);
    const blob = new Blob([bytes]);
    const url = URL.createObjectURL(blob);

    const img = new Image();
    img.onload = () => {
      if (engine.value) {
        // Clear all existing objects for a new document
        executeCommand({ action: 'clear', params: {} });
        objects.value = [];
        selectedIds.value = [];

        // Resize artboard to image size
        artboard.value.width = img.width;
        artboard.value.height = img.height;
        clipToArtboard.value = true;
        updateArtboard();

        // Center viewport and zoom to fit
        zoomToFit();

        // Add image
        const res = executeCommand({
            action: 'add',
            params: { 
                type: 'Image', 
                x: 0, 
                y: 0, 
                width: img.width, 
                height: img.height,
                name: 'Background Image'
            }
        });
        
        if (res && res.id) {
            engine.value.set_image_object(res.id, img);
            imageMap.set(res.id, img);
            
            // Send raw bytes to engine efficiently
            engine.value.set_image_raw(res.id, bytes);

            // Move to bottom and lock
            executeCommand({ action: 'move_to_back', params: { id: res.id } });
            executeCommand({ action: 'update', params: { id: res.id, locked: true } });
            needsRender.value = true;
        }
      }
    };
    img.src = url;
  };
  reader.readAsArrayBuffer(file);
}

function exportArtboard() {
    if (!canvas.value || !engine.value) return;

    // We want to render ONLY the artboard at 1:1 scale to a temporary canvas
    const tempCanvas = document.createElement('canvas');
    tempCanvas.width = artboard.value.width;
    tempCanvas.height = artboard.value.height;
    const tempCtx = tempCanvas.getContext('2d');
    
    if (tempCtx) {
        // Create a temporary engine or just manipulate the current one's viewport
        // Actually, easiest is to save current viewport, set it to 0,0 zoom 1, render, then restore
        const oldX = viewport.value.x;
        const oldY = viewport.value.y;
        const oldZoom = viewport.value.zoom;

        engine.value.set_viewport(0, 0, 1.0);
        engine.value.render(tempCtx);
        
        // Restore
        engine.value.set_viewport(oldX, oldY, oldZoom);
        needsRender.value = true;

        // Download
        const link = document.createElement('a');
        link.download = 'artboard-export.png';
        link.href = tempCanvas.toDataURL('image/png');
        link.click();
    }
}

function exportSvg() {
    if (!engine.value) return;
    const svgData = engine.value.export_svg();
    const blob = new Blob([svgData], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'export.svg';
    link.click();
    URL.revokeObjectURL(url);
}

function exportPsd() {
    if (!engine.value) return;
    const psdData = engine.value.export_psd();
    if (psdData.length === 0) return;
    const blob = new Blob([psdData], { type: 'image/vnd.adobe.photoshop' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'export.psd';
    link.click();
    URL.revokeObjectURL(url);
}

function exportAi() {
    if (!engine.value) return;
    const aiData = engine.value.export_ai();
    if (aiData.length === 0) return;
    const blob = new Blob([aiData], { type: 'application/postscript' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'export.ai';
    link.click();
    URL.revokeObjectURL(url);
}

function handleFileUpload(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) {
    console.log("No file selected");
    return;
  }
  if (!engine.value) {
    console.log("Engine not initialized");
    return;
  }

  const filename = file.name.toLowerCase();
  console.log("Handling file upload:", filename, file.size, "bytes");

  if (filename.endsWith('.psd') || filename.endsWith('.ai') || filename.endsWith('.svg')) {
      const reader = new FileReader();
      reader.onload = (e) => {
          console.log("FileReader loaded", filename);
          if (engine.value && e.target?.result instanceof ArrayBuffer) {
              const data = new Uint8Array(e.target.result);
              console.log("Data converted to Uint8Array, length:", data.length);
              try {
                  // Clear current project BEFORE import
                  executeCommand({ action: 'clear', params: {} });
                  imageMap.clear();

                  const resultJson = engine.value.import_file(file.name, data);
                  console.log("import_file returned:", resultJson.substring(0, 100) + "...");
                  const result = JSON.parse(resultJson);
                  
                  if (result.error) {
                      console.error("Import error:", result.error);
                      return;
                  }

                  let importedObjects = [];
                  if (result.objects && Array.isArray(result.objects)) {
                      importedObjects = result.objects;
                      
                      // Resize artboard
                      if (result.width && result.height) {
                          artboard.value.width = result.width;
                          artboard.value.height = result.height;
                          updateArtboard();
                          zoomToFit();
                      }
                  } else if (Array.isArray(result)) {
                      // Fallback for old format if AI import still returns array
                      importedObjects = result;
                  }

                  syncState();
                  console.log("Imported", importedObjects.length, "objects");
                  importedObjects.forEach((obj: any) => {
                      if (obj.image_data_url) {
                          console.log("Loading image for object:", obj.id, obj.name);
                          const img = new Image();
                          img.onload = () => {
                              console.log("Image loaded for object:", obj.id);
                              engine.value?.set_image_object(obj.id, img);
                              imageMap.set(obj.id, img);
                              needsRender.value = true;
                          };
                          img.onerror = (err) => {
                              console.error("Failed to load image data URL for object:", obj.id, err);
                          };
                          img.src = obj.image_data_url;
                      }
                  });
              } catch (err) {
                  console.error("Failed to import file:", err);
              }
          } else {
              console.error("FileReader result is not an ArrayBuffer or engine is null");
          }
      };
      reader.onerror = (err) => {
          console.error("FileReader error:", err);
      };
      reader.readAsArrayBuffer(file);
  } else {
      // Assume Image
    const reader = new FileReader();
    reader.onload = (e) => {
        const result = e.target?.result as ArrayBuffer;
        const bytes = new Uint8Array(result);
        const blob = new Blob([bytes]);
        const url = URL.createObjectURL(blob);
        
        const img = new Image();
        img.onload = () => {
          if (engine.value) {
            // Add object using command to get ID and struct
            const res = executeCommand({
                action: 'add',
                params: { 
                    type: 'Image', 
                    x: 50, 
                    y: 50, 
                    width: img.width / 2, 
                    height: img.height / 2,
                    save_undo: true
                }
            });
            
            if (res && res.id) {
                engine.value.set_image_object(res.id, img);
                imageMap.set(res.id, img);
                
                // Send raw bytes to engine efficiently
                engine.value.set_image_raw(res.id, bytes);

                // Ensure it's on top
                executeCommand({ action: 'move_to_front', params: { id: res.id } });
                needsRender.value = true;
            }
          }
        };
        img.src = url;
    };
    reader.readAsArrayBuffer(file);
  }
}
</script>

<template>
  <div class="app-container">
    <!-- Top Header -->
    <header class="header">
      <div class="header-left">
        <div class="logo">VECTORS <span class="pro-tag">PRO</span></div>
        <div class="menu-bar">
          <div class="menu-item">
            File
            <div class="dropdown">
              <button @click="newDocument">New</button>
              <div class="divider"></div>
              <button @click="openImageInput?.click()">Open Image...</button>
              <button @click="fileInput?.click()">Import (.ai, .psd, images)...</button>
              <div class="divider"></div>
              <button @click="showDocProps = true">Document Properties...</button>
              <div class="divider"></div>
                            <button @click="exportArtboard">Export Artboard (PNG)...</button>
                                        <button @click="exportSvg">Export SVG...</button>
                                        <button @click="exportPsd">Export PSD...</button>
                                        <button @click="exportAi">Export AI (Illustrator)...</button>
                            
                          </div>
              
          </div>
          <div class="menu-item">Edit</div>
          <div class="menu-item">Object</div>
          <div class="menu-item">View</div>
          <div class="menu-item toggle-item">
            <label class="toggle-label">
                <input type="checkbox" v-model="snapMode" />
                Snap
            </label>
          </div>
          <div class="menu-item toggle-item">
            <label class="toggle-label">
                <input type="checkbox" v-model="clipToArtboard" />
                Clip
            </label>
          </div>
          <div class="menu-item" v-if="targetImageId !== -1">
            <button @click="removeSelectedBackground" class="ai-bg-btn">
                ✨ Remove BG
            </button>
          </div>
          <div class="menu-item">
            <button @click="generativeFill" class="ai-bg-btn" style="background: linear-gradient(135deg, #667eea 0%, #764ba2 100%)">
                <Sparkles :size="14" style="margin-right: 4px" /> Generative Fill
            </button>
          </div>
        </div>
      </div>
      <div class="header-right">
        <div class="ai-status-indicator" :class="aiStatus.status">
          <div class="ai-status-dot"></div>
          <span class="ai-status-text">AI: {{ aiStatus.status === 'loading' ? aiStatus.message : aiStatus.status }}</span>
          <div v-if="aiStatus.status === 'loading'" class="ai-mini-progress">
            <div class="ai-mini-progress-fill" :style="{ width: (aiStatus.progress || 0) * 100 + '%' }"></div>
          </div>
        </div>
      </div>
    </header>

    <div class="main-layout">
      <!-- Left Toolbar -->
      <aside class="toolbar-side">
        <button :class="{ active: activeTool === 'select' }" @click="activeTool = 'select'" 
                @mouseenter="showTooltip($event, 'Select (V)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <MousePointer2 :size="18" />
        </button>

        <div class="tool-group" @mouseenter="showShapesMenu = true" @mouseleave="showShapesMenu = false">
            <button :class="{ active: ['rect', 'circle', 'star', 'poly'].includes(activeTool) }"
                    @mouseenter="showTooltip($event, 'Shapes')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                <component :is="activeTool === 'circle' ? Circle : activeTool === 'star' ? Star : activeTool === 'poly' ? Hexagon : Square" :size="18" />
            </button>
            <div v-if="showShapesMenu" class="tool-flyout">
                <button :class="{ active: activeTool === 'rect' }" @click="activeTool = 'rect'" 
                        @mouseenter="showTooltip($event, 'Rectangle (M)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Square :size="18" />
                </button>
                <button :class="{ active: activeTool === 'circle' }" @click="activeTool = 'circle'" 
                        @mouseenter="showTooltip($event, 'Circle (O)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Circle :size="18" />
                </button>
                <button :class="{ active: activeTool === 'star' }" @click="activeTool = 'star'" 
                        @mouseenter="showTooltip($event, 'Star (S)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Star :size="18" />
                </button>
                <button :class="{ active: activeTool === 'poly' }" @click="activeTool = 'poly'" 
                        @mouseenter="showTooltip($event, 'Polygon (G)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Hexagon :size="18" />
                </button>
            </div>
        </div>

        <button :class="{ active: activeTool === 'bezier' }" @click="activeTool = 'bezier'" 
                @mouseenter="showTooltip($event, 'Bezier Pen (P)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <PenTool :size="18" />
        </button>
        <button :class="{ active: activeTool === 'magic_wand' }" @click="activeTool = 'magic_wand'" 
                @mouseenter="showTooltip($event, 'Magic Wand (W)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Wand2 :size="18" />
        </button>
        <button :class="{ active: activeTool === 'clone_stamp' }" @click="activeTool = 'clone_stamp'" 
                @mouseenter="showTooltip($event, 'Clone Stamp (S)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Stamp :size="18" />
        </button>
        <button :class="{ active: activeTool === 'pencil' }" @click="activeTool = 'pencil'" 
                @mouseenter="showTooltip($event, 'Pencil (N)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Pencil :size="18" />
        </button>
        <button :class="{ active: activeTool === 'brush' }" @click="activeTool = 'brush'" 
                @mouseenter="showTooltip($event, 'Vector Brush (B)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Brush :size="18" />
        </button>
        <button :class="{ active: activeTool === 'text' }" @click="activeTool = 'text'" 
                @mouseenter="showTooltip($event, 'Text Tool (T)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Type :size="18" />
        </button>
        <button :class="{ active: activeTool === 'eyedropper' }" @click="activeTool = 'eyedropper'" 
                @mouseenter="showTooltip($event, 'Eyedropper (I)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Pipette :size="18" />
        </button>
        <button :class="{ active: activeTool === 'vectorize' }" @click="activeTool = 'vectorize'" 
                @mouseenter="showTooltip($event, 'Vectorize Image (Q)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Zap :size="18" />
        </button>
        <button :class="{ active: activeTool === 'adjustment' }" @click="activeTool = 'adjustment'" 
                @mouseenter="showTooltip($event, 'Adjustment Layer')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Settings :size="18" />
        </button>
        <button :class="{ active: activeTool === 'crop' }" @click="activeTool = 'crop'" 
                @mouseenter="showTooltip($event, 'Crop Artboard (C)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Crop :size="18" />
        </button>
        <button :class="{ active: activeTool === 'hand' }" @click="activeTool = 'hand'" 
                @mouseenter="showTooltip($event, 'Hand Tool (H)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Hand :size="18" />
        </button>
        <button :class="{ active: activeTool === 'zoom' }" @click="activeTool = 'zoom'" 
                @mouseenter="showTooltip($event, 'Zoom Tool (Z)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Search :size="18" />
        </button>
        <div class="separator"></div>
        <button @click="fileInput?.click()" 
                @mouseenter="showTooltip($event, 'Import Image or Document')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
          <Upload :size="18" />
        </button>
        <input type="file" @change="handleFileUpload" accept="image/*,.ai,.psd" ref="fileInput" style="display: none" />
        <input type="file" @change="handleOpenImage" accept="image/*" ref="openImageInput" style="display: none" />
      </aside>

      <div class="workspace-area">
        <div class="top-context-bar">
          <template v-if="targetImageId !== -1">
            <button :class="{ active: activeTool === 'eraser' }" @click="activeTool = 'eraser'" 
                    @mouseenter="showTooltip($event, 'Bitmap Eraser (E)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
              <Eraser :size="18" />
            </button>
            <div v-if="activeTool === 'eraser'" class="context-control">
              <label>Radius</label>
              <input type="range" v-model.number="eraserSize" min="2" max="100" />
            </div>
            <div v-if="activeTool === 'clone_stamp'" class="context-control">
              <label>Radius</label>
              <input type="range" v-model.number="cloneSize" min="2" max="100" />
              <span v-if="cloneSource" style="color: #4facfe; font-size: 10px; margin-left: 10px">
                Source set (Alt+Click to change)
              </span>
              <span v-else style="color: #ff5f56; font-size: 10px; margin-left: 10px">
                Alt+Click to set source
              </span>
            </div>
            <div v-if="activeTool === 'magic_wand'" class="context-control">
              <label>Tolerance</label>
              <input type="range" v-model.number="magicWandTolerance" min="1" max="255" />
              <span>{{ magicWandTolerance }}</span>
            </div>
            <div class="context-divider" v-if="selectedIds.length > 0"></div>
          </template>
          <template v-if="selectedIds.length > 0">
            <button :class="{ active: activeTool === 'rotate' }" @click="activeTool = 'rotate'" 
                    @mouseenter="showTooltip($event, 'Rotate Tool (R)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
              <RotateCw :size="18" />
            </button>
            <button :class="{ active: activeTool === 'gradient' }" @click="activeTool = 'gradient'" 
                    @mouseenter="showTooltip($event, 'Gradient Tool (G)')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
              <PaintBucket :size="18" />
            </button>
            <div class="context-divider"></div>
            <!-- Boolean Operations -->
            <div v-if="selectedIds.length >= 2" class="boolean-ops">
              <button @click="booleanOp('union')" v-tooltip="'Union (Pathfinder)'"><Square :size="16" style="background: #555; border-radius: 2px" /></button>
              <button @click="booleanOp('subtract')" v-tooltip="'Subtract (Pathfinder)'"><Square :size="16" style="clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 100%, 50% 50%, 0 50%)" /></button>
              <button @click="booleanOp('intersect')" v-tooltip="'Intersect (Pathfinder)'"><Circle :size="16" style="opacity: 0.5" /></button>
            </div>
            <!-- Masking -->
            <button v-if="selectedIds.length === 1" @click="toggleMask" :class="{ active: selectedObject?.is_mask }" v-tooltip="'Toggle as Mask'">
              <Zap :size="16" /> <!-- Placeholder icon for Mask -->
            </button>
          </template>
        </div>

        <!-- Canvas Area -->
        <main class="canvas-area" ref="canvasContainer">
          <!-- Rulers -->
          <div class="ruler-corner"></div>
          <div class="ruler ruler-horizontal" @mousedown="startGuideDrag('horizontal')">
            <div v-for="n in 100" :key="n" class="ruler-mark" :style="{ left: ((n-1)*100*viewport.zoom + viewport.x) + 'px' }">
              {{ (n-1)*100 }}
            </div>
          </div>
          <div class="ruler ruler-vertical" @mousedown="startGuideDrag('vertical')">
            <div v-for="n in 100" :key="n" class="ruler-mark" :style="{ top: ((n-1)*100*viewport.zoom + viewport.y) + 'px' }">
              {{ (n-1)*100 }}
            </div>
          </div>

          <canvas 
            ref="canvas" 
            @pointerdown="handlePointerDown"
            @pointermove="handlePointerMove"
            @pointerup="handlePointerUp"
            @pointercancel="handlePointerUp"
            @wheel.prevent="handleWheel"
            style="touch-action: none;"
          ></canvas>

        <!-- Engine Load Error Overlay -->
        <div v-if="engineLoadError" class="ai-overlay">
            <div class="ai-loader-card" style="border-color: #ff5f56;">
                <div class="ai-loader-content">
                    <div class="ai-loader-title" style="color: #ff5f56;">Engine Error</div>
                    <div style="font-size: 12px; color: #ccc; margin-bottom: 15px;">{{ engineLoadError }}</div>
                    <button @click="reloadApp()" class="ai-bg-btn-large" style="background: #444;">Reload App</button>
                </div>
            </div>
        </div>

        <!-- Floating Gradient Stop Color Picker -->
        <div v-if="activeStopWorldPos" class="floating-color-picker" :style="{ left: activeStopWorldPos.x + 'px', top: (activeStopWorldPos.y - 40) + 'px' }">
            <input 
                type="color" 
                :value="selectedObject.fill_gradient.stops[gradientState.activeStopIndex].color" 
                @input="e => updateGradientStop('fill', gradientState.activeStopIndex, 'color', (e.target as HTMLInputElement).value)"
            />
        </div>

        <!-- AI Processing Overlay -->
        <div v-if="aiStatus.status === 'loading'" class="ai-overlay">
            <div class="ai-loader-card">
                <div class="ai-loader-spinner"></div>
                <div class="ai-loader-content">
                    <div class="ai-loader-title">{{ aiStatus.message }}</div>
                    <div class="ai-progress-container">
                        <div class="ai-progress-bar" :style="{ width: (aiStatus.progress || 0) * 100 + '%' }"></div>
                    </div>
                    <div class="ai-loader-percent">{{ Math.round((aiStatus.progress || 0) * 100) }}%</div>
                </div>
            </div>
        </div>
      </main>
    </div>

      <!-- Right Panels -->
      <aside class="side-panels">
        <!-- Top Section: Properties or Layers -->
        <div class="side-panels-top">
          <!-- Brush Settings -->
          <section v-if="activeTool === 'brush'" class="panel brush-panel">
            <div class="panel-header">
                <h3>Brush Settings</h3>
                <button @click="importBrushTip" title="Import Brush Tip (PNG)" class="icon-btn">
                    <Upload :size="14" />
                </button>
            </div>
            <div class="property-grid">
              <label>Brush</label>
              <select v-model="selectedBrushId" @change="e => {
                  const id = Number((e.target as HTMLSelectElement).value);
                  if (selectedIds.length > 0) updateSelected('brush_id', id);
              }">
                  <option v-for="b in brushes" :key="b.id" :value="b.id">{{ b.name }}</option>
              </select>

              <label>Color</label>
              <div class="color-picker">
                  <input type="color" v-model="brushColor" @input="e => {
                      const val = (e.target as HTMLInputElement).value;
                      if (selectedIds.length > 0) updateSelected('fill', val);
                  }" />
                  <input type="text" v-model="brushColor" @input="e => {
                      const val = (e.target as HTMLInputElement).value;
                      if (selectedIds.length > 0) updateSelected('fill', val);
                  }" />
              </div>

              <template v-if="activeBrush">
                <label>Size</label>
                <input type="number" v-model="activeBrush.size" @input="updateBrush" />

                <label>Spacing</label>
                <input type="range" min="0.01" max="1" step="0.01" v-model="activeBrush.spacing" @input="updateBrush" />
                
                <label>Smoothing</label>
                <input type="range" min="0" max="1" step="0.1" v-model="activeBrush.smoothing" @input="updateBrush" />

                <label>Scatter</label>
                <input type="range" min="0" max="1" step="0.05" v-model="activeBrush.scatter" @input="updateBrush" />

                <label>Rotation Jitter</label>
                <input type="range" min="0" max="1" step="0.05" v-model="activeBrush.rotation_jitter" @input="updateBrush" />

                <label>Pressure</label>
                <input type="checkbox" v-model="activeBrush.pressure_enabled" @change="updateBrush" />
              </template>
            </div>
          </section>

          <!-- Vectorize Tool Settings -->
          <section v-if="activeTool === 'vectorize'" class="panel vectorize-panel">
            <h3>Vectorize Settings</h3>
            <div class="property-grid">
              <label>Sensitivity</label>
              <input type="range" min="0" max="255" step="1" v-model="vectorizeThreshold" @change="vectorizeImage(true)" />
              <div class="actions" style="grid-column: span 2;">
                  <button class="ai-bg-btn-large" @click="vectorizeImage(true)" style="background: #4facfe; color: white; width: 100%;">
                      ✨ Recompute
                  </button>
              </div>
            </div>
            <div v-if="targetImageId === -1" class="no-selection" style="padding: 12px; font-size: 11px; line-height: 1.4;">
                Click on an image in the workspace to vectorize it with the current settings.
            </div>
          </section>

          <template v-else>
            <template v-if="selectedObject">
              <section class="panel properties-panel">
            <!-- ... existing selectedObject panel ... -->
            <h3>Properties {{ selectedIds.length > 1 ? `(${selectedIds.length})` : '' }}</h3>
            <div class="property-grid">
              <label>Name</label>
              <input :value="selectedObject.name" @input="e => updateSelected('name', (e.target as HTMLInputElement).value)" />
              
              <label>X</label>
              <input type="number" :value="Math.round(selectedObject.x)" @input="e => updateSelected('x', Number((e.target as HTMLInputElement).value))" />
              
              <label>Y</label>
              <input type="number" :value="Math.round(selectedObject.y)" @input="e => updateSelected('y', Number((e.target as HTMLInputElement).value))" />
              
              <label>Width</label>
              <input type="number" :value="Math.round(selectedObject.width)" @input="e => updateSelected('width', Number((e.target as HTMLInputElement).value))" />
              
              <label>Height</label>
              <input type="number" :value="Math.round(selectedObject.height)" @input="e => updateSelected('height', Number((e.target as HTMLInputElement).value))" />
              
              <label>Rotation</label>
              <input type="number" :value="Math.round(selectedObject.rotation * 180 / Math.PI)" @input="e => updateSelected('rotation', Number((e.target as HTMLInputElement).value) * Math.PI / 180)" />

              <template v-if="selectedObject.shape_type === 'Path'">
                  <label>Brush</label>
                  <select :value="selectedObject.brush_id" @change="e => {
                      const bid = Number((e.target as HTMLSelectElement).value);
                      updateSelected('brush_id', bid);
                      if (bid > 0) selectedBrushId = bid;
                  }">
                      <option :value="0">None (Standard Stroke)</option>
                      <option v-for="b in brushes" :key="b.id" :value="b.id">{{ b.name }}</option>
                  </select>

                  <!-- Show brush settings if a brush is active on this path -->
                  <template v-if="selectedObject.brush_id > 0">
                      <template v-for="b in [brushes.find(br => br.id === selectedObject.brush_id)]" :key="b?.id">
                          <template v-if="b">
                              <div class="separator-text">BRUSH SETTINGS ({{ b.name }})</div>
                              
                              <label>Size</label>
                              <input type="number" v-model="b.size" @input="updateBrushById(b)" />

                              <label>Spacing</label>
                              <input type="range" min="0.01" max="1" step="0.01" v-model="b.spacing" @input="updateBrushById(b)" />
                              
                              <label>Scatter</label>
                              <input type="range" min="0" max="1" step="0.05" v-model="b.scatter" @input="updateBrushById(b)" />

                              <label>Rotation</label>
                              <input type="range" min="0" max="1" step="0.05" v-model="b.rotation_jitter" @input="updateBrushById(b)" />

                              <label>Pressure</label>
                              <input type="checkbox" v-model="b.pressure_enabled" @change="updateBrushById(b)" />
                          </template>
                      </template>
                  </template>
              </template>

              <label>Fill</label>
              <div class="fill-control">
                  <div class="fill-type-toggle">
                      <button :class="{ active: !selectedObject.fill_gradient }" @click="updateSelected('fill_gradient', null)">Solid</button>
                      <button :class="{ active: !!selectedObject.fill_gradient }" @click="initGradient('fill')">Gradient</button>
                  </div>
                  
                  <div v-if="!selectedObject.fill_gradient" class="color-picker">
                      <input type="color" :value="safeColor(selectedObject.fill)" @input="e => updateSelected('fill', (e.target as HTMLInputElement).value)" />
                      <input type="text" :value="selectedObject.fill" @input="e => updateSelected('fill', (e.target as HTMLInputElement).value)" />
                  </div>
                  
                  <div v-else class="gradient-editor">
                      <select :value="selectedObject.fill_gradient.is_radial ? 'radial' : 'linear'" 
                              @change="e => updateGradient('fill', 'is_radial', (e.target as HTMLSelectElement).value === 'radial')">
                          <option value="linear">Linear</option>
                          <option value="radial">Radial</option>
                      </select>
                      <div class="stops-list">
                          <div v-for="(stop, idx) in selectedObject.fill_gradient.stops" :key="idx" class="stop-item">
                              <input type="color" :value="stop.color" @input="e => updateGradientStop('fill', idx, 'color', (e.target as HTMLInputElement).value)" />
                              <input type="number" min="0" max="1" step="0.1" :value="stop.offset" @input="e => updateGradientStop('fill', idx, 'offset', Number((e.target as HTMLInputElement).value))" />
                              <button @click="removeGradientStop('fill', idx)">×</button>
                          </div>
                          <button @click="addGradientStop('fill')">+ Stop</button>
                      </div>
                      <div class="coords-editor">
                          <label>x1 <input type="number" :value="selectedObject.fill_gradient.x1" @input="e => updateGradient('fill', 'x1', Number((e.target as HTMLInputElement).value))" /></label>
                          <label>y1 <input type="number" :value="selectedObject.fill_gradient.y1" @input="e => updateGradient('fill', 'y1', Number((e.target as HTMLInputElement).value))" /></label>
                          <label>x2 <input type="number" :value="selectedObject.fill_gradient.x2" @input="e => updateGradient('fill', 'x2', Number((e.target as HTMLInputElement).value))" /></label>
                          <label>y2 <input type="number" :value="selectedObject.fill_gradient.y2" @input="e => updateGradient('fill', 'y2', Number((e.target as HTMLInputElement).value))" /></label>
                      </div>
                  </div>
              </div>

              <label>Stroke</label>
              <div class="color-picker">
                  <input type="color" :value="safeColor(selectedObject.stroke)" @input="e => updateSelected('stroke', (e.target as HTMLInputElement).value)" />
                  <input type="text" :value="selectedObject.stroke" @input="e => updateSelected('stroke', (e.target as HTMLInputElement).value)" />
              </div>

              <label>S. Width</label>
              <input type="number" :value="selectedObject.stroke_width" @input="e => updateSelected('stroke_width', Number((e.target as HTMLInputElement).value))" />

              <label>Opacity</label>
              <input 
                  type="range" min="0" max="1" step="0.1" 
                  :value="selectedObject.opacity" 
                  @input="e => updateSelected('opacity', Number((e.target as HTMLInputElement).value), false)"
                  @change="e => updateSelected('opacity', Number((e.target as HTMLInputElement).value), true)"
              />

              <label>Blend</label>
              <select :value="selectedObject.blend_mode" @change="e => updateSelected('blend_mode', (e.target as HTMLSelectElement).value)">
                  <option value="source-over">Normal</option>
                  <option value="multiply">Multiply</option>
                  <option value="screen">Screen</option>
                  <option value="overlay">Overlay</option>
                  <option value="darken">Darken</option>
                  <option value="lighten">Lighten</option>
                  <option value="color-dodge">Color Dodge</option>
                  <option value="color-burn">Color Burn</option>
                  <option value="hard-light">Hard Light</option>
                  <option value="soft-light">Soft Light</option>
                  <option value="difference">Difference</option>
                  <option value="exclusion">Exclusion</option>
                  <option value="hue">Hue</option>
                  <option value="saturation">Saturation</option>
                  <option value="color">Color</option>
                  <option value="luminosity">Luminosity</option>
              </select>

              <div class="separator-text">STROKE STYLE</div>

              <label>Cap</label>
              <select :value="selectedObject.stroke_cap" @change="e => updateSelected('stroke_cap', (e.target as HTMLSelectElement).value)">
                  <option value="butt">Butt</option>
                  <option value="round">Round</option>
                  <option value="square">Square</option>
              </select>

              <label>Join</label>
              <select :value="selectedObject.stroke_join" @change="e => updateSelected('stroke_join', (e.target as HTMLSelectElement).value)">
                  <option value="miter">Miter</option>
                  <option value="round">Round</option>
                  <option value="bevel">Bevel</option>
              </select>

              <label>Dash</label>
              <input type="text" :value="selectedObject.stroke_dash.join(',')" 
                     @input="e => updateSelected('stroke_dash', (e.target as HTMLInputElement).value.split(',').map(Number).filter(n => !isNaN(n)))" 
                     placeholder="e.g. 5,5" />

              <div class="separator-text">SHADOW</div>
              
              <label>Color</label>
              <div class="color-picker">
                  <input type="color" :value="safeColor(selectedObject.shadow_color)" @input="e => updateSelected('shadow_color', (e.target as HTMLInputElement).value)" />
                  <input type="text" :value="selectedObject.shadow_color" @input="e => updateSelected('shadow_color', (e.target as HTMLInputElement).value)" />
              </div>

              <label>Blur</label>
              <input type="number" :value="selectedObject.shadow_blur" @input="e => updateSelected('shadow_blur', Number((e.target as HTMLInputElement).value))" />

              <label>Offset X</label>
              <input type="number" :value="selectedObject.shadow_offset_x" @input="e => updateSelected('shadow_offset_x', Number((e.target as HTMLInputElement).value))" />

              <label>Offset Y</label>
              <input type="number" :value="selectedObject.shadow_offset_y" @input="e => updateSelected('shadow_offset_y', Number((e.target as HTMLInputElement).value))" />
              
              <template v-if="selectedObject.shape_type === 'Star' || selectedObject.shape_type === 'Polygon'">
                  <label>Sides</label>
                  <input type="number" :value="selectedObject.sides" @input="e => updateSelected('sides', Number((e.target as HTMLInputElement).value))" />
              </template>

              <template v-if="selectedObject.shape_type === 'Rectangle'">
                  <label>Corners</label>
                  <input type="range" min="0" max="100" step="1" :value="selectedObject.corner_radius" @input="e => updateSelected('corner_radius', Number((e.target as HTMLInputElement).value))" />
              </template>

              <template v-if="selectedObject.shape_type === 'Star'">
                  <label>Inner R.</label>
                  <input type="range" min="0" max="1" step="0.05" :value="selectedObject.inner_radius" @input="e => updateSelected('inner_radius', Number((e.target as HTMLInputElement).value))" />
              </template>

              <template v-if="selectedObject.shape_type === 'Text'">
                  <div class="separator-text">TEXT</div>
                  
                  <label>Content</label>
                  <textarea :value="selectedObject.text_content" @input="e => updateSelected('text_content', (e.target as HTMLTextAreaElement).value)" rows="3" class="text-area-input"></textarea>

                  <label>Font</label>
                  <input :value="selectedObject.font_family" @input="e => updateSelected('font_family', (e.target as HTMLInputElement).value)" />

                  <label>Size</label>
                  <input type="number" :value="selectedObject.font_size" @input="e => updateSelected('font_size', Number((e.target as HTMLInputElement).value))" />

                  <label>Weight</label>
                  <select :value="selectedObject.font_weight" @change="e => updateSelected('font_weight', (e.target as HTMLSelectElement).value)">
                      <option value="normal">Normal</option>
                      <option value="bold">Bold</option>
                      <option value="300">Light</option>
                      <option value="900">Black</option>
                  </select>

                  <label>Align</label>
                  <select :value="selectedObject.text_align" @change="e => updateSelected('text_align', (e.target as HTMLSelectElement).value)">
                      <option value="left">Left</option>
                      <option value="center">Center</option>
                      <option value="right">Right</option>
                  </select>
              </template>

              <template v-if="selectedObject.shape_type === 'Image'">
                  <div class="separator-text">FILTERS</div>
                  
                  <label>Brightness</label>
                  <input type="range" min="0" max="3" step="0.1" :value="selectedObject.brightness" 
                         @input="e => updateSelected('brightness', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('brightness', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Contrast</label>
                  <input type="range" min="0" max="3" step="0.1" :value="selectedObject.contrast" 
                         @input="e => updateSelected('contrast', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('contrast', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Saturation</label>
                  <input type="range" min="0" max="3" step="0.1" :value="selectedObject.saturate" 
                         @input="e => updateSelected('saturate', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('saturate', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Hue Rotate</label>
                  <input type="range" min="0" max="360" step="1" :value="selectedObject.hue_rotate" 
                         @input="e => updateSelected('hue_rotate', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('hue_rotate', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Blur</label>
                  <input type="range" min="0" max="20" step="0.5" :value="selectedObject.blur" 
                         @input="e => updateSelected('blur', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('blur', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Grayscale</label>
                  <input type="range" min="0" max="1" step="0.05" :value="selectedObject.grayscale" 
                         @input="e => updateSelected('grayscale', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('grayscale', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Sepia</label>
                  <input type="range" min="0" max="1" step="0.05" :value="selectedObject.sepia" 
                         @input="e => updateSelected('sepia', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('sepia', Number((e.target as HTMLInputElement).value), true)" />

                  <label>Invert</label>
                  <input type="range" min="0" max="1" step="0.05" :value="selectedObject.invert" 
                         @input="e => updateSelected('invert', Number((e.target as HTMLInputElement).value), false)"
                         @change="e => updateSelected('invert', Number((e.target as HTMLInputElement).value), true)" />

                  <div class="separator-text">VECTORIZE</div>
                  <label>Threshold</label>
                  <input type="range" min="0" max="255" step="1" v-model="vectorizeThreshold" />
                  <div class="actions">
                      <button class="ai-bg-btn-large" @click="() => vectorizeImage()" style="background: #4facfe; color: white;">✨ Trace to Path</button>
                  </div>

                  <div class="actions">
                      <button class="ai-bg-btn-large" @click="removeSelectedBackground">✨ Remove Background (AI)</button>
                  </div>
              </template>

              <label>Locked</label>
              <input type="checkbox" :checked="selectedObject.locked" @change="e => updateSelected('locked', (e.target as HTMLInputElement).checked)" />

              <div class="separator-text">ARRANGE</div>
              <div class="arrange-btns">
                  <button @click="executeCommand({ action: 'move_to_front', params: { id: selectedObject.id } })" 
                          @mouseenter="showTooltip($event, 'Bring to Front')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                      <BringToFront :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_forward', params: { id: selectedObject.id } })" 
                          @mouseenter="showTooltip($event, 'Bring Forward')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                      <ChevronUp :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_backward', params: { id: selectedObject.id } })" 
                          @mouseenter="showTooltip($event, 'Send Backward')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                      <ChevronDown :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_to_back', params: { id: selectedObject.id } })" 
                          @mouseenter="showTooltip($event, 'Send to Back')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                      <SendToBack :size="16" />
                  </button>
              </div>

              <div class="actions">
                  <button class="duplicate-btn" @click="executeCommand({ action: 'duplicate', params: { id: selectedObject.id } })"
                          @mouseenter="showTooltip($event, 'Duplicate Object')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Copy :size="14" style="margin-right: 6px; vertical-align: middle;" /> Duplicate Object
                  </button>
                  <button class="delete-btn" @click="deleteSelected"
                          @mouseenter="showTooltip($event, 'Delete Object')" @mousemove="moveTooltip" @mouseleave="hideTooltip">
                    <Trash2 :size="14" style="margin-right: 6px; vertical-align: middle;" /> Delete Object
                  </button>
              </div>
            </div>
          </section>

          <!-- Typography Panel -->
          <section v-if="selectedObject.shape_type === 'Text'" class="panel typo-panel">
            <h3>Typography</h3>
            <div class="typo-controls">
              <div class="control-group">
                <label>Leading</label>
                <input type="number" v-model="selectedObject.leading" step="0.1" @input="updateSelected('leading', selectedObject.leading)" />
              </div>
              <div class="control-group">
                <label>Tracking</label>
                <input type="number" v-model="selectedObject.tracking" @input="updateSelected('tracking', selectedObject.tracking)" />
              </div>
              <div class="control-group">
                <label>Kerning</label>
                <input type="number" v-model="selectedObject.kerning" @input="updateSelected('kerning', selectedObject.kerning)" />
              </div>
            </div>
          </section>

          <!-- Layer Style (FX) Panel -->
          <section class="panel fx-panel">
            <div class="panel-header">
              <h3>Effects (FX)</h3>
              <button @click="addEffect('DropShadow')" class="btn-icon-small"><Plus :size="14" /></button>
            </div>
            <div class="fx-list">
              <div v-for="(effect, index) in selectedObject.layer_style.effects" :key="index" class="fx-item">
                <div class="fx-row">
                  <input type="checkbox" v-model="effect.enabled" @change="updateLayerStyle" />
                  <span>{{ effect.effect_type }}</span>
                  <button @click="removeEffect(index)" class="btn-icon-small"><Trash2 :size="14" /></button>
                </div>
                <div v-if="effect.enabled" class="fx-controls">
                  <div class="control-group">
                    <label>Blur</label>
                    <input type="range" v-model="effect.blur" min="0" max="100" @input="updateLayerStyle" />
                  </div>
                  <div class="control-group">
                    <label>Offset X</label>
                    <input type="number" v-model="effect.x" @input="updateLayerStyle" />
                  </div>
                  <div class="control-group">
                    <label>Offset Y</label>
                    <input type="number" v-model="effect.y" @input="updateLayerStyle" />
                  </div>
                  <div class="control-group">
                    <label>Color</label>
                    <input type="color" v-model="effect.color" @input="updateLayerStyle" />
                  </div>
                </div>
              </div>
            </div>
          </section>
        </template>

        <section v-else-if="targetImageId !== -1" class="panel filters-panel">
            <h3>Image Adjustments (BG)</h3>
            <div class="property-grid">
                <label>Brightness</label>
                <input type="range" min="0" max="3" step="0.1" 
                       :value="objects.find(o => o.id === targetImageId)?.brightness" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, brightness: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, brightness: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <label>Contrast</label>
                <input type="range" min="0" max="3" step="0.1" 
                       :value="objects.find(o => o.id === targetImageId)?.contrast" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, contrast: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, contrast: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <label>Saturation</label>
                <input type="range" min="0" max="3" step="0.1" 
                       :value="objects.find(o => o.id === targetImageId)?.saturate" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, saturate: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, saturate: Number((e.target as HTMLInputElement).value), save_undo: true } })" />
                
                <label>Blur</label>
                <input type="range" min="0" max="20" step="0.5" 
                       :value="objects.find(o => o.id === targetImageId)?.blur" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, blur: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, blur: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <label>Grayscale</label>
                <input type="range" min="0" max="1" step="0.05" 
                       :value="objects.find(o => o.id === targetImageId)?.grayscale" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, grayscale: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, grayscale: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <label>Sepia</label>
                <input type="range" min="0" max="1" step="0.05" 
                       :value="objects.find(o => o.id === targetImageId)?.sepia" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, sepia: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, sepia: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <label>Invert</label>
                <input type="range" min="0" max="1" step="0.05" 
                       :value="objects.find(o => o.id === targetImageId)?.invert" 
                       @input="e => executeCommand({ action: 'update', params: { id: targetImageId, invert: Number((e.target as HTMLInputElement).value) } })"
                       @change="e => executeCommand({ action: 'update', params: { id: targetImageId, invert: Number((e.target as HTMLInputElement).value), save_undo: true } })" />

                <div class="separator-text" style="grid-column: span 2; margin-top: 15px;">VECTORIZE</div>
                <label>Threshold</label>
                <input type="range" min="0" max="255" step="1" v-model="vectorizeThreshold" />
                <div class="actions" style="grid-column: span 2;">
                    <button class="ai-bg-btn-large" @click="() => vectorizeImage()" style="background: #4facfe; color: white; width: 100%;">✨ Trace to Path</button>
                </div>
            </div>
          </section>

          <section v-else class="panel layers-panel">
            <h3>Layers</h3>
            <div class="layers-list">
              <LayerItem 
                v-for="obj in [...objects].reverse()" 
                :key="obj.id" 
                :obj="obj"
                :selectedIds="selectedIds"
                @select="(id, e) => {
                    if (e.shiftKey || e.metaKey) {
                        const idx = selectedIds.indexOf(id);
                        const newIds = [...selectedIds];
                        if (idx >= 0) {
                            newIds.splice(idx, 1);
                        } else {
                            newIds.push(id);
                        }
                        executeCommand({ action: 'select', params: { ids: newIds } });
                    } else {
                        executeCommand({ action: 'select', params: { id } });
                    }
                }"
                @toggle-visible="(id, v) => executeCommand({ action: 'update', params: { id, visible: v } })"
                @toggle-lock="(id, l) => executeCommand({ action: 'update', params: { id, locked: l } })"
              />
            </div>
          </section>
          </template>

          <!-- History Panel (Always Bottom of panels stack) -->
          <section class="panel history-panel">
            <h3>History</h3>
            <div class="history-list">
              <div v-for="(action, index) in history" :key="index" 
                   class="history-item" 
                   :class="{ active: index === history.length - 1 }"
                   @click="jumpToHistory(index)">
                <History :size="14" />
                <span>{{ action }}</span>
              </div>
            </div>
          </section>
        </div>

        <!-- AI Chat Panel (Always Bottom) -->
        <section class="panel ai-chat-panel">
          <h3>AI Assistant</h3>
          <div class="chat-history" ref="chatHistory">
            <div v-for="(msg, i) in messages" :key="i" :class="['message', msg.role]">
              <div class="msg-bubble">{{ msg.content }}</div>
            </div>
          </div>
          <div class="chat-input">
            <input v-model="chatInput" @keyup.enter="sendMessage" placeholder="Command AI..." />
          </div>
        </section>
      </aside>
    </div>

    <!-- Document Properties Modal -->
    <div v-if="showDocProps" class="modal-overlay">
      <div class="modal">
        <h3>Document Properties</h3>
        <div class="modal-content">
            <div class="form-group">
                <label>Width (px)</label>
                <input type="number" v-model="artboard.width">
            </div>
            <div class="form-group">
                <label>Height (px)</label>
                <input type="number" v-model="artboard.height">
            </div>
            <div class="form-group">
                <label>Background</label>
                <div class="color-picker-row">
                    <input type="color" v-model="artboard.background">
                    <input type="text" v-model="artboard.background">
                </div>
            </div>
        </div>
        <div class="modal-actions">
            <button @click="showDocProps = false" class="btn-cancel">Cancel</button>
            <button @click="updateArtboard" class="btn-save">Save</button>
        </div>
      </div>
    </div>

    <!-- Custom Tooltip -->
    <div v-if="tooltip.show" class="custom-tooltip" :style="{ left: (tooltip.x + 10) + 'px', top: (tooltip.y + 10) + 'px' }">
      {{ tooltip.text }}
    </div>
  </div>
</template>

<style scoped>
.custom-tooltip {
  position: fixed;
  background: #333;
  color: white;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 11px;
  pointer-events: none;
  z-index: 10000;
  box-shadow: 0 4px 8px rgba(0,0,0,0.3);
  white-space: nowrap;
}

:host {
  --bg-dark: #1a1a1a;
  --bg-panel: #252525;
  --border: #333;
  --accent: #4facfe;
  --text: #e0e0e0;
  --text-dim: #999;
}

.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  background: #1a1a1a;
  color: #e0e0e0;
  font-family: 'Inter', -apple-system, sans-serif;
}

.header {
  height: 48px;
  background: #252525;
  border-bottom: 1px solid #333;
  display: flex;
  align-items: center;
  padding: 0 16px;
  justify-content: space-between;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 24px;
}

.menu-bar {
  display: flex;
  gap: 2px;
  align-items: center; /* Center items in the bar */
  height: 100%;
}

.menu-item {
  position: relative;
  font-size: 13px;
  padding: 0 10px;
  height: 32px; /* Fixed height for consistent centering */
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 4px;
  color: #ccc;
  transition: background 0.1s, color 0.1s;
  line-height: 1;
}

.menu-item:hover {
  background: #333;
  color: #fff;
}

.toggle-item:hover {
    background: transparent;
}

.toggle-label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
    color: #ccc;
}

.toggle-label:hover {
    color: #fff;
}

.dropdown {
  display: none;
  position: absolute;
  top: 100%;
  left: 0;
  background: #2a2a2a;
  border: 1px solid #444;
  box-shadow: 0 4px 12px rgba(0,0,0,0.5);
  z-index: 1000;
  min-width: 180px;
  border-radius: 6px;
  padding: 6px 0;
}

/* Invisible bridge to keep menu open while moving mouse */
.dropdown::before {
    content: '';
    position: absolute;
    top: -10px;
    left: 0;
    right: 0;
    height: 10px;
}

.menu-item:hover .dropdown {
  display: block;
}

.dropdown button {
  width: 100%;
  text-align: left;
  background: transparent;
  border: none;
  color: #eee;
  padding: 8px 16px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.1s;
}

.dropdown button:hover:not(:disabled) {
  background: #4facfe;
  color: white;
}

.dropdown button:disabled {
  color: #555;
  cursor: default;
}

.divider {
  height: 1px;
  background: #444;
  margin: 6px 0;
}

.panel {
  padding: 12px;
  border-bottom: 1px solid #333;
}

.panel h3 {
  font-size: 11px;
  text-transform: uppercase;
  color: #999;
  margin-bottom: 12px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.btn-icon-small {
  background: transparent;
  border: none;
  color: #999;
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
  border-radius: 4px;
}

.btn-icon-small:hover {
  background: #444;
  color: #fff;
}

/* History Panel */
.history-list {
  max-height: 200px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.history-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  font-size: 12px;
  color: #bbb;
  cursor: pointer;
  border-radius: 4px;
}

.history-item:hover {
  background: #333;
}

.history-item.active {
  background: rgba(79, 172, 254, 0.2);
  color: #4facfe;
}

/* FX Panel */
.fx-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.fx-item {
  background: #1e1e1e;
  border-radius: 4px;
  padding: 4px;
}

.fx-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  padding: 4px;
}

.fx-controls {
  padding: 8px;
  border-top: 1px solid #333;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.typo-controls {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.control-group {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  color: #999;
}

.control-group input[type="range"] {
  width: 60px;
}

.control-group input[type="number"] {
  width: 40px;
  background: #111;
  border: 1px solid #444;
  color: #eee;
  font-size: 10px;
}

.boolean-ops {
  display: flex;
  gap: 4px;
  margin: 0 8px;
}

.boolean-ops button {
  background: #333;
  border: 1px solid #444;
  color: #eee;
  padding: 4px;
  border-radius: 4px;
  cursor: pointer;
}

.boolean-ops button:hover {
  background: #4facfe;
  border-color: #4facfe;
}

.pro-tag {
  background: #4facfe;
  color: white;
  font-size: 10px;
  font-weight: 700;
  padding: 2px 5px;
  border-radius: 4px;
  vertical-align: middle;
  letter-spacing: 0.5px;
}

.ai-status-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: #999;
  background: #1a1a1a;
  padding: 4px 10px;
  border-radius: 12px;
  border: 1px solid #333;
}

.ai-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #555;
}

.ai-status-indicator.ready .ai-status-dot { background: #a8ff78; box-shadow: 0 0 5px #a8ff78; }
.ai-status-indicator.loading .ai-status-dot { background: #ffca28; box-shadow: 0 0 5px #ffca28; }
.ai-status-indicator.error .ai-status-dot { background: #ff5f56; }

.ai-mini-progress {
  width: 40px;
  height: 4px;
  background: #333;
  border-radius: 2px;
  overflow: hidden;
}

.ai-mini-progress-fill {
  height: 100%;
  background: #4facfe;
  transition: width 0.3s ease;
}

/* AI Overlay */
.ai-overlay {
    position: absolute;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.ai-loader-card {
    background: #252525;
    border: 1px solid #444;
    border-radius: 12px;
    padding: 24px;
    display: flex;
    align-items: center;
    gap: 20px;
    box-shadow: 0 20px 40px rgba(0,0,0,0.6);
    min-width: 320px;
}

.ai-loader-spinner {
    width: 40px;
    height: 40px;
    border: 3px solid #333;
    border-top: 3px solid #4facfe;
    border-radius: 50%;
    animation: spin 1s linear infinite;
}

.ai-loader-content {
    flex: 1;
}

.ai-loader-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 8px;
    color: #eee;
}

.ai-progress-container {
    height: 6px;
    background: #1a1a1a;
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 4px;
}

.ai-progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #4facfe, #00f2fe);
    transition: width 0.3s ease;
}

.ai-loader-percent {
    font-size: 11px;
    color: #888;
    text-align: right;
}

@keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

.main-layout {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.workspace-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.top-context-bar {
    height: 40px;
    min-height: 40px;
    background: #252525;
    border-bottom: 1px solid #333;
    display: flex;
    align-items: center;
    padding: 0 10px;
    gap: 8px;
}

.top-context-bar button {
    width: 32px;
    height: 32px;
    background: transparent;
    border: none;
    color: #999;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
}

.top-context-bar button:hover {
    background: #333;
    color: white;
}

.top-context-bar button.active {
    background: #4facfe;
    color: white;
}

.context-divider {
    width: 1px;
    height: 20px;
    background: #333;
    margin: 0 10px;
}

.context-control {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: #999;
    padding-left: 4px;
}

.context-control input[type="range"] {
    width: 80px;
    height: 4px;
    accent-color: #4facfe;
}

/* Toolbar */
.toolbar-side {
  width: 45px;
  background: #252525;
  border-right: 1px solid #333;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 10px 0;
  gap: 8px;
}

.toolbar-side button {
  width: 32px;
  height: 32px;
  background: transparent;
  border: none;
  color: #999;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.2s;
  overflow: hidden;
}

.toolbar-side button:hover {
  background: #333;
  color: white;
}

.toolbar-side button.active {
  background: #4facfe;
  color: white;
}

.tool-group {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.tool-flyout {
    position: absolute;
    left: 100%;
    top: 0;
    background: #252525;
    border: 1px solid #333;
    border-radius: 0 6px 6px 0;
    display: flex;
    flex-direction: row;
    padding: 4px;
    gap: 4px;
    z-index: 100;
    box-shadow: 4px 0 12px rgba(0,0,0,0.5);
}

.separator {
  width: 20px;
  height: 1px;
  background: #333;
  margin: 5px 0;
}

/* Canvas Area */
.canvas-area {
  flex: 1;
  background: #111;
  position: relative;
  overflow: hidden;
  /* Grid pattern */
  background-image: radial-gradient(#222 1px, transparent 1px);
  background-size: 20px 20px;
}

.canvas-area canvas {
  position: absolute;
  top: 30px;
  left: 30px;
  width: calc(100% - 30px);
  height: calc(100% - 30px);
}

.ruler-corner {
  position: absolute;
  top: 0;
  left: 0;
  width: 30px;
  height: 30px;
  background: #252525;
  z-index: 11;
  border-right: 1px solid #333;
  border-bottom: 1px solid #333;
}

.ruler {
  position: absolute;
  background: #252525;
  z-index: 10;
  font-size: 9px;
  color: #666;
  user-select: none;
}

.ruler-horizontal {
  top: 0;
  left: 30px;
  right: 0;
  height: 30px;
  border-bottom: 1px solid #333;
}

.ruler-vertical {
  top: 30px;
  left: 0;
  bottom: 0;
  width: 30px;
  border-right: 1px solid #333;
}

.ruler-mark {
  position: absolute;
  padding: 2px;
}

.ruler-horizontal .ruler-mark {
  border-left: 1px solid #444;
  height: 100%;
}

.ruler-vertical .ruler-mark {
  border-top: 1px solid #444;
  width: 100%;
}


canvas {
  display: block;
}

/* Side Panels */
.side-panels {
  width: 280px;
  background: #252525;
  border-left: 1px solid #333;
  display: flex;
  flex-direction: column;
  height: 100%;
}

.side-panels-top {
    flex: 1;
    overflow-y: auto;
    min-height: 0; /* Important for flex child scrolling */
}

.panel {
  border-bottom: 1px solid #333;
  display: flex;
  flex-direction: column;
}

.panel h3 {
  font-size: 11px;
  text-transform: uppercase;
  color: #666;
  padding: 8px 12px;
  margin: 0;
  background: #2a2a2a;
  position: sticky;
  top: 0;
  z-index: 10;
}

.brush-panel .property-grid {
    border-bottom: 1px solid #333;
}

.ai-chat-panel {
  flex: 0 0 auto;
  max-height: 60%;
  background: #252525;
  border-top: 1px solid #333;
  border-bottom: none;
}

.chat-history {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  min-height: 100px;
  max-height: 300px;
}

.property-grid {
  display: grid;
  grid-template-columns: 70px 1fr;
  gap: 8px;
  padding: 12px;
  font-size: 12px;
  align-items: center;
}

.property-grid input, .property-grid select, .property-grid textarea {
  background: #1a1a1a;
  border: 1px solid #333;
  color: #eee;
  padding: 4px 8px;
  border-radius: 4px;
  font-family: inherit;
  font-size: 12px;
}

.text-area-input {
    grid-column: span 2;
    resize: vertical;
    min-height: 60px;
}

.separator-text {
    grid-column: span 2;
    font-size: 9px;
    font-weight: 800;
    color: #555;
    padding: 10px 0 5px 0;
    border-bottom: 1px solid #333;
    margin-bottom: 5px;
}

.color-picker {
    display: flex;
    gap: 4px;
}

.color-picker input[type="color"] {
    width: 30px;
    padding: 0;
    height: 24px;
}

.actions {
    grid-column: span 2;
    padding-top: 10px;
}

.arrange-btns {
    grid-column: span 2;
    display: flex;
    gap: 4px;
}

.arrange-btns button {
    flex: 1;
    background: #333;
    border: 1px solid #444;
    color: #eee;
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
}

.arrange-btns button:hover {
    background: #444;
}

.ai-bg-btn {
    background: #4facfe;
    color: white;
    border: none;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    font-weight: 600;
    transition: background 0.2s;
}

.ai-bg-btn:hover {
    background: #0088ff;
}

.ai-bg-btn-large {
    width: 100%;
    background: #4facfe;
    color: white;
    border: none;
    padding: 8px;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: background 0.2s;
}

.ai-bg-btn-large:hover {
    background: #0088ff;
}

.duplicate-btn {
    width: 100%;
    background: #333;
    color: #eee;
    border: 1px solid #444;
    padding: 6px;
    border-radius: 4px;
    cursor: pointer;
    margin-bottom: 8px;
}

.duplicate-btn:hover {
    background: #444;
}

.delete-btn {
    width: 100%;
    background: #442222;
    color: #ff6666;
    border: 1px solid #663333;
    padding: 6px;
    border-radius: 4px;
    cursor: pointer;
}

.layers-list {
  overflow-y: auto;
}

.layer-item {
  padding: 8px 12px;
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  border-bottom: 1px solid #2a2a2a;
}

.layer-item:hover { background: #2a2a2a; }
.layer-item.selected { background: #333; border-left: 2px solid #4facfe; }

.visibility-toggle {
    background: transparent;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 14px;
    padding: 0;
    width: 20px;
    text-align: center;
}

.visibility-toggle:hover {
    color: #eee;
}

.layer-icon { color: #666; font-size: 14px; }

.lock-toggle {
    margin-left: auto;
    background: transparent;
    border: none;
    cursor: pointer;
    font-size: 12px;
    opacity: 0.5;
    transition: opacity 0.2s;
}

.lock-toggle:hover {
    opacity: 1;
}

.message {
  margin-bottom: 8px;
  display: flex;
}

.message.user { justify-content: flex-end; }

.msg-bubble {
  padding: 6px 12px;
  border-radius: 12px;
  font-size: 12px;
  max-width: 85%;
}

.user .msg-bubble { background: #333; color: #eee; }
.ai .msg-bubble { background: #2a2a2a; color: #a8ff78; border: 1px solid #333; }

.chat-input {
  padding: 12px;
  background: #252525;
}

.chat-input input {
  width: 100%;
  background: #1a1a1a;
  border: 1px solid #333;
  color: white;
  padding: 10px 12px;
  border-radius: 6px;
  box-sizing: border-box; /* Fix for overflow */
  outline: none;
  transition: border-color 0.2s;
}

.chat-input input:focus {
    border-color: #4facfe;
}
.fill-control {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
}

.fill-type-toggle {
    display: flex;
    gap: 4px;
}

.fill-type-toggle button {
    flex: 1;
    background: #333;
    border: 1px solid #444;
    color: #999;
    padding: 4px;
    cursor: pointer;
    font-size: 10px;
}

.fill-type-toggle button.active {
    background: #4facfe;
    color: white;
}

.gradient-editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: #2a2a2a;
    padding: 8px;
    border-radius: 4px;
}

.stops-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.stop-item {
    display: flex;
    gap: 4px;
    align-items: center;
}

.stop-item input[type="color"] {
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
}

.stop-item input[type="number"] {
    width: 40px;
}

.coords-editor {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
}

.coords-editor label {
    display: flex;
    gap: 4px;
    align-items: center;
    font-size: 10px;
}

.coords-editor input {
    width: 100%;
}

.no-selection {
    padding: 20px;
    text-align: center;
    color: #555;
    font-size: 12px;
    font-style: italic;
}

.floating-color-picker {
    position: absolute;
    z-index: 1000;
    pointer-events: none;
    opacity: 0;
    width: 1px;
    height: 1px;
}

.floating-color-picker input[type="color"] {
    width: 1px;
    height: 1px;
    padding: 0;
    border: none;
    pointer-events: auto;
}

.modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
}
.modal {
    background: #252525;
    border: 1px solid #444;
    border-radius: 6px;
    width: 300px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.5);
}
.modal h3 {
    margin: 0;
    padding: 10px 15px;
    background: #2a2a2a;
    border-bottom: 1px solid #333;
    font-size: 13px;
    text-transform: uppercase;
    color: #888;
}
.modal-content {
    padding: 15px;
}
.form-group {
    margin-bottom: 12px;
}
.form-group label {
    display: block;
    margin-bottom: 5px;
    font-size: 12px;
    color: #aaa;
}
.form-group input {
    width: 100%;
    background: #1a1a1a;
    border: 1px solid #333;
    color: white;
    padding: 6px;
    border-radius: 4px;
}
.color-picker-row {
    display: flex;
    gap: 8px;
}
.color-picker-row input[type="color"] {
    width: 40px;
    padding: 0;
    height: 30px;
}
.modal-actions {
    padding: 10px 15px;
    background: #2a2a2a;
    border-top: 1px solid #333;
    display: flex;
    justify-content: flex-end;
    gap: 10px;
}
.btn-cancel, .btn-save {
    padding: 6px 12px;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    font-size: 12px;
}
.btn-cancel {
    background: transparent;
    color: #aaa;
    border: 1px solid #444;
}
.btn-save {
    background: #4facfe;
    color: white;
}
</style>