<script setup lang="ts">
import { ref, onMounted, computed, watch, nextTick } from 'vue';
import init, { VectorEngine } from './pkg/engine';
import { aiService, type ModelStatus } from './ai';

type Tool = 'select' | 'rect' | 'circle' | 'image' | 'bezier' | 'crop' | 'star' | 'poly' | 'eyedropper' | 'magic';

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
const selectedId = ref<number>(-1);
const isDragging = ref(false);
const dragStart = ref({ x: 0, y: 0 });
const lastMousePos = ref({ x: 0, y: 0 });
const initialObjectState = ref<{ x: number, y: number, width: number, height: number } | null>(null);
const needsRender = ref(true);

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
    points: [] as BezierPoint[],
    currentObjId: -1,
    isSnapped: false,
});

function getPathString(points: BezierPoint[]): string {
    if (points.length === 0) return '';
    let d = `M ${points[0].x} ${points[0].y}`;
    for (let i = 1; i < points.length; i++) {
        const prev = points[i-1];
        const curr = points[i];
        d += ` C ${prev.cout.x} ${prev.cout.y}, ${curr.cin.x} ${curr.cin.y}, ${curr.x} ${curr.y}`;
    }
    return d;
}

const hasImage = computed(() => {
    return objects.value.some(o => o.shape_type === 'Image');
});

const selectedObject = computed(() => {
  return objects.value.find(o => o.id === selectedId.value) || null;
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
        if (e.key.toLowerCase() === 'r') activeTool.value = 'rect';
        if (e.key.toLowerCase() === 'o') activeTool.value = 'circle';
        if (e.key.toLowerCase() === 's') activeTool.value = 'star';
        if (e.key.toLowerCase() === 'g') activeTool.value = 'poly';
        if (e.key.toLowerCase() === 'p') activeTool.value = 'bezier';
        if (e.key.toLowerCase() === 'i') activeTool.value = 'eyedropper';
        if (e.key.toLowerCase() === 'm') activeTool.value = 'magic';
        if (e.key.toLowerCase() === 'c' && hasImage.value) activeTool.value = 'crop';
        if (e.key === 'Backspace' || e.key === 'Delete') deleteSelected();

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
             // ... existing enter logic ...
             const pts = bezierState.value.points;
            if (pts.length > 0) {
                let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
                pts.forEach(p => {
                    minX = Math.min(minX, p.x, p.cin.x, p.cout.x);
                    minY = Math.min(minY, p.y, p.cin.y, p.cout.y);
                    maxX = Math.max(maxX, p.x, p.cin.x, p.cout.x);
                    maxY = Math.max(maxY, p.y, p.cin.y, p.cout.y);
                });
                
                const shiftedPts = pts.map(p => ({
                    x: p.x - minX,
                    y: p.y - minY,
                    cin: { x: p.cin.x - minX, y: p.cin.y - minY },
                    cout: { x: p.cout.x - minX, y: p.cout.y - minY }
                }));
                
                const newD = getPathString(shiftedPts);
                executeCommand({
                    action: 'update',
                    params: {
                        id: bezierState.value.currentObjId,
                        x: minX,
                        y: minY,
                        width: maxX - minX,
                        height: maxY - minY,
                        path_data: newD
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

        // Sync objects state for UI
        const json = engine.value.get_objects_json();
        objects.value = JSON.parse(json);
        selectedId.value = engine.value.get_selected_id();
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
  e.preventDefault();

  const rect = canvas.value.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;

  if (e.ctrlKey) {
    // Pinch to zoom (macOS trackpad)
    const zoomSensitivity = 0.005;
    const delta = -e.deltaY * zoomSensitivity;
    const newZoom = Math.max(0.01, Math.min(100, viewport.value.zoom * Math.exp(delta)));

    // World pos before zoom
    const wx = (mx - viewport.value.x) / viewport.value.zoom;
    const wy = (my - viewport.value.y) / viewport.value.zoom;

    viewport.value.zoom = newZoom;
    viewport.value.x = mx - wx * newZoom;
    viewport.value.y = my - wy * newZoom;
  } else {
    // Two-finger pan (macOS trackpad)
    viewport.value.x -= e.deltaX;
    viewport.value.y -= e.deltaY;
  }

  engine.value.set_viewport(viewport.value.x, viewport.value.y, viewport.value.zoom);
  needsRender.value = true;
}

// Mouse Interactions
function handleMouseDown(e: MouseEvent) {
  if (!canvas.value || !engine.value) return;
  const rect = canvas.value.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  
  dragStart.value = { x, y };
  lastMousePos.value = { x, y };

  if (isSpacePressed.value) {
      isPanning.value = true;
      canvas.value.style.cursor = 'grabbing';
      return;
  }

  isDragging.value = true;
  needsRender.value = true;

  const worldPos = screenToWorld(x, y);

  if (activeTool.value === 'bezier') {
      if (!bezierState.value.isDrawing) {
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
                bezierState.value.points = [{ x: worldPos.x, y: worldPos.y, cin: {x: worldPos.x, y: worldPos.y}, cout: {x: worldPos.x, y: worldPos.y} }];
                engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
            }
      } else {
          // Check for Snap-Close
          let isClose = false;
          if (snapMode.value && bezierState.value.points.length > 2) {
              const startPt = bezierState.value.points[0];
              const dist = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
              if (dist < 15 / viewport.value.zoom) {
                  isClose = true;
              }
          }

          if (isClose) {
                // ... same closing logic ...
                const pts = bezierState.value.points;
                let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
                pts.forEach(p => {
                    minX = Math.min(minX, p.x, p.cin.x, p.cout.x);
                    minY = Math.min(minY, p.y, p.cin.y, p.cout.y);
                    maxX = Math.max(maxX, p.x, p.cin.x, p.cout.x);
                    maxY = Math.max(maxY, p.y, p.cin.y, p.cout.y);
                });
                
                const shiftedPts = pts.map(p => ({
                    x: p.x - minX,
                    y: p.y - minY,
                    cin: { x: p.cin.x - minX, y: p.cin.y - minY },
                    cout: { x: p.cout.x - minX, y: p.cout.y - minY }
                }));
                
                const newD = getPathString(shiftedPts) + " Z";
                executeCommand({
                    action: 'update',
                    params: {
                        id: bezierState.value.currentObjId,
                        x: minX,
                        y: minY,
                        width: maxX - minX,
                        height: maxY - minY,
                        path_data: newD,
                        save_undo: true
                    }
                });
                
                bezierState.value.isDrawing = false;
                bezierState.value.points = [];
                activeTool.value = 'select';
          } else {
              // Add point
              bezierState.value.points.push({ x: worldPos.x, y: worldPos.y, cin: {x: worldPos.x, y: worldPos.y}, cout: {x: worldPos.x, y: worldPos.y} });
              const d = getPathString(bezierState.value.points);
              executeCommand({
                    action: 'update',
                    params: { id: bezierState.value.currentObjId, path_data: d }
                });
          }
      }
      return;
  }

  if (activeTool.value === 'select') {
    engine.value.select_at(x, y);
    const sid = engine.value.get_selected_id();
    if (sid !== -1) {
        const obj = objects.value.find(o => o.id === sid);
        if (obj) {
            initialObjectState.value = { x: obj.x, y: obj.y, width: obj.width, height: obj.height };
        }
    } else {
        initialObjectState.value = null;
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
    }
  } else if (activeTool.value === 'star' || activeTool.value === 'poly') {
    const type = activeTool.value === 'star' ? 'Star' : 'Polygon';
    const res = executeCommand({
        action: 'add',
        params: { type, x: worldPos.x, y: worldPos.y, width: 1, height: 1, fill: '#4facfe', sides: 5 }
    });
    if (res.id) {
        engine.value.execute_command(JSON.stringify({ action: 'select', params: { id: res.id } }));
    }
  } else if (activeTool.value === 'eyedropper') {
      const ctx = canvas.value.getContext('2d');
      if (ctx) {
          // We need to sample from the ACTUAL canvas pixels
          const pixel = ctx.getImageData(x, y, 1, 1).data;
          const hex = "#" + ((1 << 24) + (pixel[0] << 16) + (pixel[1] << 8) + pixel[2]).toString(16).slice(1);
          if (selectedId.value !== -1) {
              updateSelected('fill', hex);
          }
      }
  } else if (activeTool.value === 'magic') {
      // Magic tool selects and asks AI to "do something" with this area
      engine.value.select_at(x, y);
      const sid = engine.value.get_selected_id();
      if (sid !== -1) {
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

  const worldPos = screenToWorld(x, y);

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
  
  if (!isDragging.value && !cropState.value.isCropping) return;
  
  if (activeTool.value === 'crop' && cropState.value.isCropping) {
    cropState.value.currentX = worldPos.x;
    cropState.value.currentY = worldPos.y;
    needsRender.value = true;
    return;
  }

  if (activeTool.value === 'bezier' && bezierState.value.isDrawing) {
      const idx = bezierState.value.points.length - 1;
      if (idx >= 0) {
          const pt = bezierState.value.points[idx];
          
          let targetX = worldPos.x;
          let targetY = worldPos.y;
          bezierState.value.isSnapped = false;

          // Snap Logic
          if (snapMode.value && bezierState.value.points.length > 2) {
              const startPt = bezierState.value.points[0];
              const dist = Math.sqrt((worldPos.x - startPt.x)**2 + (worldPos.y - startPt.y)**2);
              if (dist < 15 / viewport.value.zoom) { // Snap threshold
                  targetX = startPt.x;
                  targetY = startPt.y;
                  bezierState.value.isSnapped = true;
              }
          }

          pt.x = targetX;
          pt.y = targetY;
          pt.cout = { x: targetX, y: targetY };
          pt.cin = { 
              x: pt.x - (worldPos.x - pt.x),
              y: pt.y - (worldPos.y - pt.y) 
          };
          
          if (bezierState.value.isSnapped) {
               pt.cin = { x: targetX, y: targetY };
               pt.cout = { x: targetX, y: targetY };
          }

          const d = getPathString(bezierState.value.points);
          executeCommand({
                action: 'update',
                params: { id: bezierState.value.currentObjId, path_data: d }
            });
      }
      return;
  }

  if (activeTool.value === 'select' && selectedObject.value && initialObjectState.value) {
    const startWorld = screenToWorld(dragStart.value.x, dragStart.value.y);
    const dx = worldPos.x - startWorld.x;
    const dy = worldPos.y - startWorld.y;

    executeCommand({
        action: 'update',
        params: { 
            id: selectedObject.value.id, 
            x: initialObjectState.value.x + dx, 
            y: initialObjectState.value.y + dy 
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

function handleMouseUp() {
  if (isPanning.value) {
      isPanning.value = false;
      if (canvas.value) canvas.value.style.cursor = isSpacePressed.value ? 'grab' : 'default';
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

  if (activeTool.value === 'select' && isDragging.value && selectedObject.value) {
      // Create undo point at the end of dragging
      executeCommand({
          action: 'update',
          params: { id: selectedObject.value.id, save_undo: true }
      });
  }

  isDragging.value = false;
  if (!['select', 'bezier'].includes(activeTool.value)) {
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
      // Map AI commands to our unified action system
      let action = cmd.command === 'add' ? 'add' : cmd.command === 'move' ? 'update' : cmd.command;
      if (action === 'move') action = 'update';
      
      executeCommand({ action, params: cmd.params });
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

function updateSelected(key: string, value: any, saveUndo: boolean = true) {
    if (selectedId.value === -1) return;
    executeCommand({
        action: 'update',
        params: { id: selectedId.value, [key]: value, save_undo: saveUndo }
    });
}

function deleteSelected() {
    if (selectedId.value === -1) return;
    executeCommand({ action: 'delete', params: { id: selectedId.value } });
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
        selectedId.value = -1;

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
          <span class="icon">↗</span>
        </button>

        <div class="tool-group" @mouseenter="showShapesMenu = true" @mouseleave="showShapesMenu = false">
            <button :class="{ active: ['rect', 'circle', 'star', 'poly'].includes(activeTool) }" title="Shapes">
                <span class="icon">{{ 
                    activeTool === 'circle' ? '○' : 
                    activeTool === 'star' ? '★' : 
                    activeTool === 'poly' ? '⬢' : '▢' 
                }}</span>
            </button>
            <div v-if="showShapesMenu" class="tool-flyout">
                <button :class="{ active: activeTool === 'rect' }" @click="activeTool = 'rect'" title="Rectangle (R)">
                    <span class="icon">▢</span>
                </button>
                <button :class="{ active: activeTool === 'circle' }" @click="activeTool = 'circle'" title="Circle (O)">
                    <span class="icon">○</span>
                </button>
                <button :class="{ active: activeTool === 'star' }" @click="activeTool = 'star'" title="Star (S)">
                    <span class="icon">★</span>
                </button>
                <button :class="{ active: activeTool === 'poly' }" @click="activeTool = 'poly'" title="Polygon (G)">
                    <span class="icon">⬢</span>
                </button>
            </div>
        </div>

        <button :class="{ active: activeTool === 'bezier' }" @click="activeTool = 'bezier'" title="Bezier Pen (P)">
          <span class="icon">✒</span>
        </button>
        <button :class="{ active: activeTool === 'eyedropper' }" @click="activeTool = 'eyedropper'" title="Eyedropper (I)">
          <span class="icon">💧</span>
        </button>
        <button :class="{ active: activeTool === 'magic' }" @click="activeTool = 'magic'" title="Magic AI (M)">
          <span class="icon">✨</span>
        </button>
        <button :class="{ active: activeTool === 'crop' }" @click="activeTool = 'crop'" title="Crop Artboard (C)">
          <span class="icon">✂</span>
        </button>
        <div class="separator"></div>
        <button @click="fileInput?.click()" title="Import Image or Document">
          <span class="icon">🖼</span>
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
          @wheel="handleWheel"
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
            <h3>Properties</h3>
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
              
              <label>Fill</label>
              <div class="color-picker">
                  <input type="color" :value="safeColor(selectedObject.fill)" @input="e => updateSelected('fill', (e.target as HTMLInputElement).value)" />
                  <input type="text" :value="selectedObject.fill" @input="e => updateSelected('fill', (e.target as HTMLInputElement).value)" />
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
              
              <template v-if="selectedObject.shape_type === 'Star' || selectedObject.shape_type === 'Polygon'">
                  <label>Sides</label>
                  <input type="number" :value="selectedObject.sides" @input="e => updateSelected('sides', Number((e.target as HTMLInputElement).value))" />
              </template>

              <template v-if="selectedObject.shape_type === 'Star'">
                  <label>Inner R.</label>
                  <input type="range" min="0" max="1" step="0.05" :value="selectedObject.inner_radius" @input="e => updateSelected('inner_radius', Number((e.target as HTMLInputElement).value))" />
              </template>

              <template v-if="selectedObject.shape_type === 'Image'">
                  <div class="actions">
                      <button class="ai-bg-btn-large" @click="removeSelectedBackground">✨ Remove Background (AI)</button>
                  </div>
              </template>

              <label>Locked</label>
              <input type="checkbox" :checked="selectedObject.locked" @change="e => updateSelected('locked', (e.target as HTMLInputElement).checked)" />

              <div class="actions">
                  <button class="delete-btn" @click="deleteSelected">Delete Object</button>
              </div>
            </div>
          </section>

          <section v-else class="panel layers-panel">
            <h3>Layers</h3>
            <div class="layers-list">
              <div 
                v-for="obj in [...objects].reverse()" 
                :key="obj.id" 
                :class="['layer-item', { selected: obj.id === selectedId }]"
                @click="engine?.execute_command(JSON.stringify({ action: 'select', params: { id: obj.id } }))"
              >
                <span class="layer-icon">
                    {{ 
                        obj.shape_type === 'Rectangle' ? '▢' : 
                        obj.shape_type === 'Circle' ? '○' : 
                        obj.shape_type === 'Ellipse' ? '⬭' : 
                        obj.shape_type === 'Star' ? '★' : 
                        obj.shape_type === 'Polygon' ? '⬢' : 
                        obj.shape_type === 'Image' ? '🖼' : 
                        obj.shape_type === 'Path' ? '✒' : '?' 
                    }}
                </span>
                <span class="layer-name">{{ obj.name }}</span>
                <button 
                  class="lock-toggle" 
                  @click.stop="executeCommand({ action: 'update', params: { id: obj.id, locked: !obj.locked } })"
                  title="Toggle Lock"
                >
                  {{ obj.locked ? '🔒' : '🔓' }}
                </button>
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
  height: 40px;
  background: #252525;
  border-bottom: 1px solid #333;
  display: flex;
  align-items: center;
  padding: 0 15px;
  justify-content: space-between;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 20px;
}

.menu-bar {
  display: flex;
  gap: 4px;
}

.menu-item {
  position: relative;
  font-size: 12px;
  padding: 4px 8px;
  cursor: pointer;
  border-radius: 4px;
}

.menu-item:hover {
  background: #333;
}

.toggle-item:hover {
    background: transparent;
}

.toggle-label {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    user-select: none;
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
  min-width: 160px;
  border-radius: 4px;
  padding: 4px 0;
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
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}

.dropdown button:hover:not(:disabled) {
  background: #4facfe;
}

.dropdown button:disabled {
  color: #555;
  cursor: default;
}

.divider {
  height: 1px;
  background: #444;
  margin: 4px 0;
}

.logo {
  font-weight: 800;
  letter-spacing: 1px;
  font-size: 14px;
}

.pro-tag {
  background: #4facfe;
  color: white;
  font-size: 9px;
  padding: 1px 4px;
  border-radius: 3px;
  vertical-align: middle;
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

.property-grid input {
  background: #1a1a1a;
  border: 1px solid #333;
  color: #eee;
  padding: 4px 8px;
  border-radius: 4px;
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
  padding: 10px;
}

.chat-input input {
  width: 100%;
  background: #1a1a1a;
  border: 1px solid #333;
  color: white;
  padding: 8px 12px;
  border-radius: 20px;
  font-size: 12px;
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