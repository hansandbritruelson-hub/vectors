<script setup lang="ts">
import { ref, onMounted, computed, watch, nextTick } from 'vue';
import init, { VectorEngine } from './pkg/engine';
import { aiService, type ModelStatus } from './ai';
import LayerItem from './components/LayerItem.vue';
import { 
    MousePointer2, Square, Circle, PenTool, Crop, 
    Star, Hexagon, Pipette, Wand2, Type, Upload,
    Trash2, Copy, BringToFront, SendToBack, ChevronUp, ChevronDown,
    Pencil, Eraser, Hand, Search, RotateCw, PaintBucket
} from 'lucide-vue-next';

type Tool = 'select' | 'rect' | 'circle' | 'image' | 'bezier' | 'crop' | 'star' | 'poly' | 'eyedropper' | 'magic' | 'text' | 'pencil' | 'eraser' | 'hand' | 'zoom' | 'rotate' | 'gradient';

const canvas = ref<HTMLCanvasElement | null>(null);
const canvasContainer = ref<HTMLElement | null>(null);
const chatHistory = ref<HTMLElement | null>(null);
const engine = ref<VectorEngine | null>(null);
const chatInput = ref('');
const messages = ref<{ role: string, content: string }[]>([]);
const fileInput = ref<HTMLInputElement | null>(null);
const openImageInput = ref<HTMLInputElement | null>(null);
const aiStatus = ref<ModelStatus>({ status: 'idle', message: 'AI ready to load' });

// State
const activeTool = ref<Tool>('select');
const showShapesMenu = ref(false);
const snapMode = ref(true);
const objects = ref<any[]>([]);
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


watch(messages, () => {
    nextTick(() => {
        if (chatHistory.value) {
            chatHistory.value.scrollTop = chatHistory.value.scrollHeight;
        }
    });
}, { deep: true });

const viewport = ref({ x: 50, y: 50, zoom: 1.0 });
const isPanning = ref(false);
const isSpacePressed = ref(false);
const artboard = ref({ width: 800, height: 600, background: '#ffffff' });
const clipToArtboard = ref(false);
const showDocProps = ref(false);

function screenToWorld(x: number, y: number) {
    if (!canvas.value) return { x, y };
    return {
        x: (x - viewport.value.x) / viewport.value.zoom,
        y: (y - viewport.value.y) / viewport.value.zoom
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
    if (selectedObject.value && selectedObject.value.shape_type === 'Image') {
        return selectedObject.value.id;
    }
    const bottomImage = objects.value.find(o => o.shape_type === 'Image');
    return bottomImage ? bottomImage.id : -1;
});

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
        const newImg = new Image();
        newImg.onload = () => {
            if (engine.value) {
                engine.value.set_image_object(id, newImg);
                imageMap.set(id, newImg);
                needsRender.value = true;
                executeCommand({ action: 'update', params: { id, save_undo: true } });
            }
        };
        newImg.src = url;
    } catch (e) {
        console.error("Failed to remove background:", e);
    }
}

watch(clipToArtboard, (val) => {
    executeCommand({ action: 'set_clipping', params: { enabled: val } });
});

onMounted(async () => {
  await init();
  
  if (canvas.value && canvasContainer.value) {
    canvas.value.width = canvasContainer.value.clientWidth;
    canvas.value.height = canvasContainer.value.clientHeight;
    
    engine.value = new VectorEngine();
    updateArtboard();
    executeCommand({ action: 'add', params: { type: 'Rectangle', x: 100, y: 100, width: 200, height: 150, fill: '#4facfe' } });
    
    window.addEventListener('resize', handleResize);
    window.addEventListener('keydown', (e) => {
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

        if (e.code === 'Space' && !isSpacePressed.value) {
             isSpacePressed.value = true;
             canvas.value!.style.cursor = 'grab';
        }
        
        // Tool Shortcuts
        if (e.key.toLowerCase() === 'v') activeTool.value = 'select';
        if (e.key.toLowerCase() === 'm') activeTool.value = 'rect';
        if (e.key.toLowerCase() === 'r') activeTool.value = 'rotate';
        if (e.key.toLowerCase() === 'o') activeTool.value = 'circle';
        if (e.key.toLowerCase() === 's') activeTool.value = 'star';
        if (e.key.toLowerCase() === 'g') activeTool.value = 'poly';
        if (e.key.toLowerCase() === 'p') activeTool.value = 'bezier';
        if (e.key.toLowerCase() === 'n') activeTool.value = 'pencil';
        if (e.key.toLowerCase() === 'e') activeTool.value = 'eraser';
        if (e.key.toLowerCase() === 'h') activeTool.value = 'hand';
        if (e.key.toLowerCase() === 'z') activeTool.value = 'zoom';
        if (e.key.toLowerCase() === 'i') activeTool.value = 'eyedropper';
        if (e.key.toLowerCase() === 'm') activeTool.value = 'magic';
        if (e.key.toLowerCase() === 't') activeTool.value = 'text';
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
    });
    window.addEventListener('keyup', (e) => {
        if (e.code === 'Space') {
            isSpacePressed.value = false;
            isPanning.value = false;
            canvas.value!.style.cursor = 'default';
        }
    });

    // Init Viewport
    engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);

    renderLoop();
  }

  aiService.onStatusUpdate = (status) => {
    aiStatus.value = status;
  };
});

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

function newDocument() {
    if (!engine.value) return;
    // We could add a clear_all command to the engine, or just re-instantiate
    engine.value = new VectorEngine();
    // Re-sync artboard and viewport
    updateArtboard();
    viewport.value = { x: 50, y: 50, zoom: 1.0 };
    engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
    needsRender.value = true;
}

function handleResize() {
  if (canvas.value && canvasContainer.value) {
    canvas.value.width = canvasContainer.value.clientWidth;
    canvas.value.height = canvasContainer.value.clientHeight;
    needsRender.value = true;
  }
}

function renderLoop() {
  if (!engine.value || !canvas.value) return;
  
  if (needsRender.value) {
      const ctx = canvas.value.getContext('2d', { willReadFrequently: true });
      if (ctx) {
        engine.value.render(ctx);

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
        if (activeTool.value === 'bezier' && (bezierState.value.isDrawing || bezierState.value.isEditing)) {
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

        // Sync objects state for UI
        const json = engine.value.get_objects_json();
        objects.value = JSON.parse(json);
        const selIdsJson = engine.value.get_selected_ids();
        selectedIds.value = JSON.parse(selIdsJson);
      }
      needsRender.value = false;
  }
  requestAnimationFrame(renderLoop);
}

function executeCommand(cmd: any) {
  if (!engine.value) return;
  const result = engine.value.execute_command(JSON.stringify(cmd));
  const parsed = JSON.parse(result);
  if (parsed.error) console.error("Command Error:", parsed.error);
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
  
  // Mouse Interactions
  function handleMouseDown(e: MouseEvent) {  if (!canvas.value || !engine.value) return;
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

  isDragging.value = true; // Default to true, might set to false if box select
  needsRender.value = true;

  const worldPos = screenToWorld(x, y);

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
          engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
      }
      return;
  }

  if (activeTool.value === 'eraser') {
      const idsStr = engine.value.select_point(x, y, false);
      const ids = JSON.parse(idsStr);
      if (ids.length > 0) {
          executeCommand({ action: 'delete', params: { ids } });
      }
      return;
  }

  if (activeTool.value === 'gradient') {
      if (!selectedObject.value) {
          const idsStr = engine.value.select_point(x, y, false);
          selectedIds.value = JSON.parse(idsStr);
          return;
      }
      
      const obj = selectedObject.value;
      if (!obj.fill_gradient) {
          // If clicked on object, init gradient
          const idsStr = engine.value.select_point(x, y, false);
          const ids = JSON.parse(idsStr);
          if (ids.includes(obj.id)) {
               initGradient('fill');
          }
          return;
      }

      const grad = obj.fill_gradient;
      const p1 = localToWorld(obj, grad.x1, grad.y1);
      const p2 = localToWorld(obj, grad.x2, grad.y2);
      const hitDist = 8 / viewport.value.zoom;

      // Start/End
      if (Math.hypot(worldPos.x - p1.x, worldPos.y - p1.y) < hitDist) {
          gradientState.value = { isDragging: true, dragType: 'start', dragIndex: -1, activeStopIndex: -1 };
          return;
      }
      if (Math.hypot(worldPos.x - p2.x, worldPos.y - p2.y) < hitDist) {
          gradientState.value = { isDragging: true, dragType: 'end', dragIndex: -1, activeStopIndex: -1 };
          return;
      }

      // Stops
      if (grad.stops) {
          for (let i = 0; i < grad.stops.length; i++) {
              const stop = grad.stops[i];
              const sx = p1.x + (p2.x - p1.x) * stop.offset;
              const sy = p1.y + (p2.y - p1.y) * stop.offset;
              if (Math.hypot(worldPos.x - sx, worldPos.y - sy) < hitDist) {
                  gradientState.value = { isDragging: true, dragType: 'stop', dragIndex: i, activeStopIndex: i };
                  return;
              }
          }
      }

      // Add Stop
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
                   gradientState.value = { isDragging: true, dragType: 'stop', dragIndex: newIdx, activeStopIndex: newIdx };
                   return;
              }
          }
      }
      
      const idsStr = engine.value.select_point(x, y, false);
      selectedIds.value = JSON.parse(idsStr);
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
          const idsStr = engine.value.select_point(x, y, false);
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
                engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
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
    const idsStr = engine.value.select_point(x, y, e.shiftKey || e.metaKey);
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
  } else if (activeTool.value === 'rect' || activeTool.value === 'circle') {
    const type = activeTool.value === 'rect' ? 'Rectangle' : 'Circle';
    const res = executeCommand({
        action: 'add',
        params: { type, x: worldPos.x, y: worldPos.y, width: 1, height: 1, fill: '#4facfe' }
    });
    if (res.id) {
        engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
        // Force update state immediately so mousemove can see it
        const json = engine.value.get_objects_json();
        objects.value = JSON.parse(json);
        selectedIds.value = [res.id];
    }
  } else if (activeTool.value === 'star' || activeTool.value === 'poly') {
    const type = activeTool.value === 'star' ? 'Star' : 'Polygon';
    const res = executeCommand({
        action: 'add',
        params: { type, x: worldPos.x, y: worldPos.y, width: 1, height: 1, fill: '#4facfe', sides: 5 }
    });
    if (res.id) {
        engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
        // Force update state immediately
        const json = engine.value.get_objects_json();
        objects.value = JSON.parse(json);
        selectedIds.value = [res.id];
    }
  } else if (activeTool.value === 'text') {
    const res = executeCommand({
        action: 'add',
        params: { type: 'Text', x: worldPos.x, y: worldPos.y, width: 200, height: 40, fill: '#000000' }
    });
    if (res.id) {
        engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
        // Force update state immediately
        const json = engine.value.get_objects_json();
        objects.value = JSON.parse(json);
        selectedIds.value = [res.id];
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
  } else if (activeTool.value === 'magic') {
      // Magic tool selects and asks AI to "do something" with this area
      engine.value.select_point(x, y, false);
      const sid = engine.value.get_selected_ids(); // returns JSON string
      const ids = JSON.parse(sid);
      if (ids.length > 0) {
          chatInput.value = "Improve this object";
          sendMessage();
      } else {
          chatInput.value = "Add something interesting here";
          sendMessage();
      }
  }
}

function handleMouseMove(e: MouseEvent) {
  if (!canvas.value || !engine.value) return;
  const rect = canvas.value.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;

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

  if (activeTool.value === 'pencil') {
      canvas.value.style.cursor = 'crosshair';
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

  if (activeTool.value === 'eraser' && isDragging.value) {
      canvas.value.style.cursor = 'crosshair'; // Or an eraser-like cursor
      const idsStr = engine.value.select_point(x, y, false);
      const ids = JSON.parse(idsStr);
      if (ids.length > 0) {
          executeCommand({ action: 'delete', params: { ids } });
      }
      return;
  }

  if (activeTool.value === 'gradient') {
      canvas.value.style.cursor = 'default';
      
      if (gradientState.value.isDragging && selectedObject.value && selectedObject.value.fill_gradient) {
          const obj = selectedObject.value;
          const grad = obj.fill_gradient;
          const type = gradientState.value.dragType;
          
          if (type === 'start') {
              const localPt = worldToLocal(obj, worldPos.x, worldPos.y);
              updateGradient('fill', 'x1', localPt.x, false);
              updateGradient('fill', 'y1', localPt.y, false);
          } else if (type === 'end') {
              const localPt = worldToLocal(obj, worldPos.x, worldPos.y);
              updateGradient('fill', 'x2', localPt.x, false);
              updateGradient('fill', 'y2', localPt.y, false);
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

  if (activeTool.value === 'zoom') {
      canvas.value.style.cursor = e.altKey ? 'zoom-out' : 'zoom-in';
  }

  if (activeTool.value === 'bezier') {
      canvas.value.style.cursor = 'crosshair';
      if (bezierState.value.isDrawing && !isDragging.value && snapMode.value && bezierState.value.points.length > 2) {
          const startPt = bezierState.value.points[0];
          const dist = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
          if (dist < 15 / viewport.value.zoom) {
              canvas.value.style.cursor = 'pointer';
          }
      }
  } else {
      canvas.value.style.cursor = 'default';
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

  if (activeTool.value === 'bezier') {
      const pts = bezierState.value.points;
      
      // EDIT MODE
      if (bezierState.value.isEditing && isDragging.value && bezierState.value.dragIndex !== -1) {
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

function handleMouseUp(e: MouseEvent) {
  if (isPanning.value) {
      isPanning.value = false;
      if (canvas.value) canvas.value.style.cursor = isSpacePressed.value ? 'grab' : 'default';
  }

  if (selectionBox.value) {
      try {
          const { x, y, w, h } = selectionBox.value;
          if (Math.abs(w) > 2 && Math.abs(h) > 2) {
              const idsStr = engine.value?.select_rect(x, y, w, h, e.shiftKey || e.metaKey);
              if (idsStr) selectedIds.value = JSON.parse(idsStr);
          }
      } finally {
          selectionBox.value = null;
          needsRender.value = true;
      }
      // We don't return here because we might need to reset tools or other cleanup
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

  if (activeTool.value === 'bezier' && bezierState.value.isEditing) {
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
  
  if (!['select', 'bezier', 'pencil', 'eraser', 'hand', 'zoom', 'rotate'].includes(activeTool.value)) {
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

function updateSelected(key: string, value: any, saveUndo: boolean = true) {
    if (selectedIds.value.length === 0) return;
    executeCommand({
        action: 'update',
        params: { ids: selectedIds.value, [key]: value, save_undo: saveUndo }
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
        const padding = 50;
        const container = canvasContainer.value;
        if (container) {
            const zoomX = (container.clientWidth - padding * 2) / img.width;
            const zoomY = (container.clientHeight - padding * 2) / img.height;
            const zoom = Math.min(zoomX, zoomY, 1.0);
            
            const vx = (container.clientWidth - img.width * zoom) / 2;
            const vy = (container.clientHeight - img.height * zoom) / 2;
            
            viewport.value = { x: vx, y: vy, zoom };
            engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
        }

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
            // Move to bottom and lock
            executeCommand({ action: 'move_to_back', params: { id: res.id } });
            executeCommand({ action: 'update', params: { id: res.id, locked: true } });
            needsRender.value = true;
        }
      }
    };
    img.src = e.target?.result as string;
  };
  reader.readAsDataURL(file);
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

function handleFileUpload(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file || !engine.value) return;

  const filename = file.name.toLowerCase();

  if (filename.endsWith('.psd') || filename.endsWith('.ai')) {
      const reader = new FileReader();
      reader.onload = (e) => {
          if (engine.value && e.target?.result instanceof ArrayBuffer) {
              const data = new Uint8Array(e.target.result);
              try {
                  const resultJson = engine.value.import_file(file.name, data);
                  const result = JSON.parse(resultJson);
                  
                  if (result.error) {
                      console.error("Import error:", result.error);
                      return;
                  }

                  // Result is an array of objects.
                  // For each object, if it has image_data_url, we need to load it.
                  if (Array.isArray(result)) {
                      result.forEach((obj: any) => {
                          if (obj.image_data_url) {
                              const img = new Image();
                              img.onload = () => {
                                  engine.value?.set_image_object(obj.id, img);
                                  imageMap.set(obj.id, img);
                                  needsRender.value = true;
                              };
                              img.src = obj.image_data_url;
                          }
                      });
                  }
              } catch (err) {
                  console.error("Failed to import file:", err);
              }
          }
      };
      reader.readAsArrayBuffer(file);
  } else {
      // Assume Image
      const reader = new FileReader();
      reader.onload = (e) => {
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
                // Ensure it's on top
                executeCommand({ action: 'move_to_front', params: { id: res.id } });
                needsRender.value = true;
            }
          }
        };
        img.src = e.target?.result as string;
      };
      reader.readAsDataURL(file);
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
              <button @click="exportArtboard">Export Artboard...</button>
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
        <button :class="{ active: activeTool === 'select' }" @click="activeTool = 'select'" title="Select (V)">
          <MousePointer2 :size="18" />
        </button>

        <div class="tool-group" @mouseenter="showShapesMenu = true" @mouseleave="showShapesMenu = false">
            <button :class="{ active: ['rect', 'circle', 'star', 'poly'].includes(activeTool) }" title="Shapes">
                <component :is="activeTool === 'circle' ? Circle : activeTool === 'star' ? Star : activeTool === 'poly' ? Hexagon : Square" :size="18" />
            </button>
            <div v-if="showShapesMenu" class="tool-flyout">
                <button :class="{ active: activeTool === 'rect' }" @click="activeTool = 'rect'" title="Rectangle (M)">
                    <Square :size="18" />
                </button>
                <button :class="{ active: activeTool === 'circle' }" @click="activeTool = 'circle'" title="Circle (O)">
                    <Circle :size="18" />
                </button>
                <button :class="{ active: activeTool === 'star' }" @click="activeTool = 'star'" title="Star (S)">
                    <Star :size="18" />
                </button>
                <button :class="{ active: activeTool === 'poly' }" @click="activeTool = 'poly'" title="Polygon (G)">
                    <Hexagon :size="18" />
                </button>
            </div>
        </div>

        <button :class="{ active: activeTool === 'rotate' }" @click="activeTool = 'rotate'" title="Rotate Tool (R)">
          <RotateCw :size="18" />
        </button>

        <button :class="{ active: activeTool === 'gradient' }" @click="activeTool = 'gradient'" title="Gradient Tool (G)">
          <PaintBucket :size="18" />
        </button>

        <button :class="{ active: activeTool === 'bezier' }" @click="activeTool = 'bezier'" title="Bezier Pen (P)">
          <PenTool :size="18" />
        </button>
        <button :class="{ active: activeTool === 'pencil' }" @click="activeTool = 'pencil'" title="Pencil (N)">
          <Pencil :size="18" />
        </button>
        <button :class="{ active: activeTool === 'eraser' }" @click="activeTool = 'eraser'" title="Eraser (E)">
          <Eraser :size="18" />
        </button>
        <button :class="{ active: activeTool === 'text' }" @click="activeTool = 'text'" title="Text Tool (T)">
          <Type :size="18" />
        </button>
        <button :class="{ active: activeTool === 'eyedropper' }" @click="activeTool = 'eyedropper'" title="Eyedropper (I)">
          <Pipette :size="18" />
        </button>
        <button :class="{ active: activeTool === 'magic' }" @click="activeTool = 'magic'" title="Magic AI (M)">
          <Wand2 :size="18" />
        </button>
        <button :class="{ active: activeTool === 'crop' }" @click="activeTool = 'crop'" title="Crop Artboard (C)">
          <Crop :size="18" />
        </button>
        <button :class="{ active: activeTool === 'hand' }" @click="activeTool = 'hand'" title="Hand Tool (H)">
          <Hand :size="18" />
        </button>
        <button :class="{ active: activeTool === 'zoom' }" @click="activeTool = 'zoom'" title="Zoom Tool (Z)">
          <Search :size="18" />
        </button>
        <div class="separator"></div>
        <button @click="fileInput?.click()" title="Import Image or Document">
          <Upload :size="18" />
        </button>
        <input type="file" @change="handleFileUpload" accept="image/*,.ai,.psd" ref="fileInput" style="display: none" />
        <input type="file" @change="handleOpenImage" accept="image/*" ref="openImageInput" style="display: none" />
      </aside>

      <!-- Canvas Area -->
      <main class="canvas-area" ref="canvasContainer">
        <canvas 
          ref="canvas" 
          @mousedown="handleMouseDown"
          @mousemove="handleMouseMove"
          @mouseup="handleMouseUp"
          @mouseleave="handleMouseUp"
          @wheel.prevent="handleWheel"
        ></canvas>

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

      <!-- Right Panels -->
      <aside class="side-panels">
        <!-- Top Section: Properties or Layers -->
        <div class="side-panels-top">
          <section v-if="selectedObject" class="panel properties-panel">
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

                  <div class="actions">
                      <button class="ai-bg-btn-large" @click="removeSelectedBackground">✨ Remove Background (AI)</button>
                  </div>
              </template>

              <label>Locked</label>
              <input type="checkbox" :checked="selectedObject.locked" @change="e => updateSelected('locked', (e.target as HTMLInputElement).checked)" />

              <div class="separator-text">ARRANGE</div>
              <div class="arrange-btns">
                  <button @click="executeCommand({ action: 'move_to_front', params: { id: selectedObject.id } })" title="Bring to Front">
                      <BringToFront :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_forward', params: { id: selectedObject.id } })" title="Bring Forward">
                      <ChevronUp :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_backward', params: { id: selectedObject.id } })" title="Send Backward">
                      <ChevronDown :size="16" />
                  </button>
                  <button @click="executeCommand({ action: 'move_to_back', params: { id: selectedObject.id } })" title="Send to Back">
                      <SendToBack :size="16" />
                  </button>
              </div>

              <div class="actions">
                  <button class="duplicate-btn" @click="executeCommand({ action: 'duplicate', params: { id: selectedObject.id } })">
                    <Copy :size="14" style="margin-right: 6px; vertical-align: middle;" /> Duplicate Object
                  </button>
                  <button class="delete-btn" @click="deleteSelected">
                    <Trash2 :size="14" style="margin-right: 6px; vertical-align: middle;" /> Delete Object
                  </button>
              </div>
            </div>
          </section>

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
  </div>
</template>

<style scoped>
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
  margin-top: 4px;
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

.logo {
  font-weight: 800;
  letter-spacing: 0.5px;
  font-size: 16px;
  color: #fff;
  display: flex;
  align-items: center;
  gap: 6px;
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