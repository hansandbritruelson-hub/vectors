<template>
  <div class="brush-studio-overlay" @click.self="$emit('close')">
    <div class="brush-studio-modal">
      <div class="studio-header">
        <h2>Brush Studio</h2>
        <button class="close-btn" @click="$emit('close')">×</button>
      </div>

      <div class="studio-content">
        <!-- Sidebar: Brush List -->
        <div class="brush-list">
          <div class="list-header">
            <span>Brushes</span>
            <button class="add-btn" @click="createNewBrush">+</button>
          </div>
          <div 
            v-for="brush in brushes" 
            :key="brush.id" 
            class="brush-item"
            :class="{ active: selectedId === brush.id }"
            @click="selectBrush(brush.id)"
          >
            <div class="brush-preview-thumb">
              <!-- Simple CSS circle or image preview -->
              <div v-if="isImageTip(brush)" class="thumb-img" :style="getThumbStyle(brush)"></div>
              <div v-else class="thumb-shape" :style="getShapeStyle(brush)"></div>
            </div>
            <span class="brush-name">{{ brush.name }}</span>
          </div>
        </div>

        <!-- Main Area: Settings & Preview -->
        <div class="settings-area" v-if="activeBrush">
          <div class="preview-panel">
            <canvas ref="scratchpad" width="400" height="150" 
              @pointerdown="startStroke" 
              @pointermove="drawStroke" 
              @pointerup="endStroke"
              @pointerleave="endStroke"
            ></canvas>
            <div class="preview-hint">Scratchpad: Test your brush here</div>
          </div>

          <div class="controls-scroll">
            <!-- Section: General -->
            <div class="control-group">
              <h3>General</h3>
              <div class="control-row">
                <label>Name</label>
                <input type="text" v-model="activeBrush.name" @change="update">
              </div>
              <div class="control-row">
                <label>Size ({{ activeBrush.size.toFixed(1) }}px)</label>
                <input type="range" min="1" max="200" step="1" v-model.number="activeBrush.size" @input="update">
              </div>
              <div class="control-row">
                <label>Opacity</label>
                <!-- Note: Opacity is usually handled by color, but we can have max opacity here if engine supports it. 
                     Engine brush struct doesn't have opacity, so we skip or map it to color alpha if needed. 
                     For now, let's assume it's controlled by the color picker in main app. -->
                <span class="info">Controlled by Color Picker</span>
              </div>
            </div>

            <!-- Section: Shape -->
            <div class="control-group">
              <h3>Shape</h3>
              <div class="control-row">
                <label>Type</label>
                <select :value="getTipType(activeBrush)" @change="e => setTipType((e.target as HTMLSelectElement).value)">
                  <option value="Calligraphic">Calligraphic</option>
                  <option value="Image">Image</option>
                </select>
              </div>

              <div v-if="getTipType(activeBrush) === 'Calligraphic'">
                <div class="control-row">
                  <label>Roundness ({{ getTipRoundness(activeBrush).toFixed(2) }})</label>
                  <input type="range" min="0.05" max="1.0" step="0.05" :value="getTipRoundness(activeBrush)" @input="e => setTipRoundness(Number((e.target as HTMLInputElement).value))">
                </div>
                <div class="control-row">
                  <label>Angle ({{ (getTipAngle(activeBrush) * 180 / Math.PI).toFixed(0) }}°)</label>
                  <input type="range" min="0" max="360" step="1" :value="getTipAngle(activeBrush) * 180 / Math.PI" @input="e => setTipAngle(Number((e.target as HTMLInputElement).value) * Math.PI / 180)">
                </div>
              </div>

              <div v-if="getTipType(activeBrush) === 'Image'">
                 <div class="control-row">
                    <label>Tip Image</label>
                    <button @click="$emit('import-tip')">Import Image...</button>
                 </div>
                 <div class="tip-image-id">ID: {{ activeBrush.tip.Image.image_id }}</div>
              </div>
            </div>

            <!-- Section: Stroke Path -->
            <div class="control-group">
              <h3>Stroke Path</h3>
              <div class="control-row">
                <label>Spacing ({{ (activeBrush.spacing * 100).toFixed(0) }}%)</label>
                <input type="range" min="0.01" max="1.0" step="0.01" v-model.number="activeBrush.spacing" @input="update">
              </div>
              <div class="control-row">
                <label>Smoothing ({{ (activeBrush.smoothing * 100).toFixed(0) }}%)</label>
                <input type="range" min="0" max="1.0" step="0.05" v-model.number="activeBrush.smoothing" @input="update">
              </div>
            </div>

            <!-- Section: Dynamics -->
            <div class="control-group">
              <h3>Dynamics</h3>
              <div class="control-row">
                <label>Scatter ({{ (activeBrush.scatter * 100).toFixed(0) }}%)</label>
                <input type="range" min="0" max="2.0" step="0.05" v-model.number="activeBrush.scatter" @input="update">
              </div>
              <div class="control-row">
                <label>Rotation Jitter ({{ (activeBrush.rotation_jitter * 100).toFixed(0) }}%)</label>
                <input type="range" min="0" max="1.0" step="0.05" v-model.number="activeBrush.rotation_jitter" @input="update">
              </div>
            </div>

            <!-- Section: Pressure -->
            <div class="control-group">
              <h3>Pressure</h3>
              <div class="control-row">
                <label>
                  <input type="checkbox" v-model="activeBrush.pressure_enabled" @change="update">
                  Enable Pressure Size
                </label>
              </div>
              <div class="control-row" v-if="activeBrush.pressure_enabled">
                <label>Min Size ({{ (activeBrush.min_size_fraction * 100).toFixed(0) }}%)</label>
                <input type="range" min="0" max="1.0" step="0.05" v-model.number="activeBrush.min_size_fraction" @input="update">
              </div>
            </div>

          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';

const props = defineProps<{
  brushes: any[],
  selectedId: number
}>();

const emit = defineEmits(['update:selectedId', 'update-brush', 'create-brush', 'import-tip', 'close']);

const activeBrush = ref<any>(null);
const scratchpad = ref<HTMLCanvasElement | null>(null);

// Scratchpad state
const isDrawing = ref(false);
const lastPos = ref({ x: 0, y: 0 });

watch(() => props.selectedId, (newId) => {
  const b = props.brushes.find(b => b.id === newId);
  if (b) activeBrush.value = JSON.parse(JSON.stringify(b)); // Deep copy to avoid mutating prop directly before save
  clearScratchpad();
}, { immediate: true });

watch(() => props.brushes, (newBrushes) => {
  if (activeBrush.value) {
    // If the active brush was updated externally, refresh our copy
    const b = newBrushes.find(b => b.id === activeBrush.value.id);
    if (b) activeBrush.value = JSON.parse(JSON.stringify(b));
  }
}, { deep: true });

function selectBrush(id: number) {
  emit('update:selectedId', id);
}

function createNewBrush() {
  emit('create-brush');
}

function update() {
  if (activeBrush.value) {
    emit('update-brush', activeBrush.value);
  }
}

// Helpers for Tip Enum which is Rust-style { Calligraphic: {...} } or { Image: {...} }
function getTipType(brush: any) {
  if (brush.tip.Calligraphic) return 'Calligraphic';
  if (brush.tip.Image) return 'Image';
  return 'Calligraphic';
}

function setTipType(type: string) {
  if (type === 'Calligraphic') {
    activeBrush.value.tip = { Calligraphic: { angle: 0, roundness: 1.0 } };
  } else {
    activeBrush.value.tip = { Image: { image_id: 'default' } };
  }
  update();
}

function getTipRoundness(brush: any) {
  return brush.tip.Calligraphic?.roundness || 1.0;
}

function setTipRoundness(val: number) {
  if (activeBrush.value.tip.Calligraphic) {
    activeBrush.value.tip.Calligraphic.roundness = val;
    update();
  }
}

function getTipAngle(brush: any) {
  return brush.tip.Calligraphic?.angle || 0;
}

function setTipAngle(val: number) {
  if (activeBrush.value.tip.Calligraphic) {
    activeBrush.value.tip.Calligraphic.angle = val;
    update();
  }
}

function isImageTip(brush: any) {
  return !!brush.tip.Image;
}

function getThumbStyle(_brush: any) {
  // We can't easily show the image blob here without a URL map passed in,
  // so for now we'll just show a gray box or placeholder.
  return { backgroundColor: '#888' };
}

function getShapeStyle(brush: any) {
  const round = brush.tip.Calligraphic?.roundness || 1.0;
  const angle = brush.tip.Calligraphic?.angle || 0;
  return {
    width: '20px',
    height: `${20 * round}px`,
    transform: `rotate(${angle}rad)`,
    backgroundColor: '#000',
    borderRadius: '50%'
  };
}

// Scratchpad Logic
function clearScratchpad() {
  if (scratchpad.value) {
    const ctx = scratchpad.value.getContext('2d');
    if (ctx) {
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, scratchpad.value.width, scratchpad.value.height);
    }
  }
}

function startStroke(e: PointerEvent) {
  isDrawing.value = true;
  lastPos.value = { x: e.offsetX, y: e.offsetY };
  drawDot(e.offsetX, e.offsetY, e.pressure);
}

function endStroke() {
  isDrawing.value = false;
}

function drawStroke(e: PointerEvent) {
  if (!isDrawing.value) return;
  
  const x = e.offsetX;
  const y = e.offsetY;
  const pressure = e.pressure;
  
  const dist = Math.hypot(x - lastPos.value.x, y - lastPos.value.y);
  const step = Math.max(1, activeBrush.value.size * activeBrush.value.spacing);
  
  if (dist >= step) {
    // Interpolate
    const steps = Math.floor(dist / step);
    const dx = (x - lastPos.value.x) / steps;
    const dy = (y - lastPos.value.y) / steps;
    
    for (let i = 1; i <= steps; i++) {
       drawDot(lastPos.value.x + dx * i, lastPos.value.y + dy * i, pressure);
    }
    lastPos.value = { x, y };
  }
}

function drawDot(x: number, y: number, pressure: number) {
  if (!scratchpad.value || !activeBrush.value) return;
  const ctx = scratchpad.value.getContext('2d');
  if (!ctx) return;
  
  ctx.save();
  ctx.translate(x, y);
  
  // Scatter
  if (activeBrush.value.scatter > 0) {
      const offset = activeBrush.value.size * activeBrush.value.scatter * 5;
      ctx.translate((Math.random() - 0.5) * offset, (Math.random() - 0.5) * offset);
  }

  // Rotation Jitter
  if (activeBrush.value.rotation_jitter > 0) {
      ctx.rotate((Math.random() - 0.5) * 2 * Math.PI * activeBrush.value.rotation_jitter);
  }

  let size = activeBrush.value.size;
  if (activeBrush.value.pressure_enabled) {
      const minFrac = activeBrush.value.min_size_fraction;
      size = size * (minFrac + (1.0 - minFrac) * pressure);
  }
  
  const tip = activeBrush.value.tip;
  if (tip.Calligraphic) {
      ctx.rotate(tip.Calligraphic.angle);
      ctx.scale(1.0, tip.Calligraphic.roundness);
      ctx.beginPath();
      ctx.arc(0, 0, size / 2, 0, Math.PI * 2);
      ctx.fillStyle = '#000000';
      ctx.fill();
  } else if (tip.Image) {
      // Placeholder for image brush preview
      ctx.fillStyle = '#000000';
      ctx.fillRect(-size/2, -size/2, size, size);
  }
  
  ctx.restore();
}

onMounted(() => {
  clearScratchpad();
});
</script>

<style scoped>
.brush-studio-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 2000;
  backdrop-filter: blur(4px);
}

.brush-studio-modal {
  width: 800px;
  height: 600px;
  background: #1e1e1e;
  color: #fff;
  border-radius: 12px;
  box-shadow: 0 10px 40px rgba(0,0,0,0.5);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  font-family: 'Inter', sans-serif;
}

.studio-header {
  padding: 16px 24px;
  background: #252525;
  border-bottom: 1px solid #333;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.studio-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.close-btn {
  background: none;
  border: none;
  color: #888;
  font-size: 24px;
  cursor: pointer;
}
.close-btn:hover { color: #fff; }

.studio-content {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.brush-list {
  width: 220px;
  background: #252525;
  border-right: 1px solid #333;
  display: flex;
  flex-direction: column;
}

.list-header {
  padding: 12px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #333;
  font-size: 12px;
  font-weight: 600;
  color: #888;
}

.add-btn {
  background: none;
  border: 1px solid #444;
  color: #fff;
  border-radius: 4px;
  width: 24px;
  height: 24px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.add-btn:hover { background: #333; }

.brush-item {
  padding: 10px 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  transition: background 0.2s;
}
.brush-item:hover { background: #2a2a2a; }
.brush-item.active { background: #4facfe; color: #fff; }

.brush-preview-thumb {
  width: 30px;
  height: 30px;
  background: #fff;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.thumb-img { width: 100%; height: 100%; background-size: cover; }

.settings-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
}

.preview-panel {
  height: 180px;
  background: #f0f0f0; /* Light background for drawing check */
  border-bottom: 1px solid #333;
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  overflow: hidden;
}

.preview-hint {
  position: absolute;
  bottom: 8px;
  right: 8px;
  color: #888;
  font-size: 10px;
  pointer-events: none;
}

canvas {
  cursor: crosshair;
}

.controls-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.control-group {
  margin-bottom: 24px;
  background: #252525;
  padding: 16px;
  border-radius: 8px;
}

.control-group h3 {
  margin: 0 0 16px 0;
  font-size: 14px;
  font-weight: 600;
  color: #ccc;
  border-bottom: 1px solid #333;
  padding-bottom: 8px;
}

.control-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.control-row label {
  font-size: 13px;
  color: #bbb;
}

.info {
  font-size: 11px;
  color: #666;
  font-style: italic;
}

input[type="range"] {
  width: 140px;
}

input[type="text"] {
  background: #333;
  border: 1px solid #444;
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
}

select {
  background: #333;
  border: 1px solid #444;
  color: #fff;
  padding: 4px 8px;
  border-radius: 4px;
}

.tip-image-id {
  font-size: 10px;
  color: #666;
  margin-top: 4px;
  text-align: right;
}
</style>
