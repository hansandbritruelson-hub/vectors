<script setup lang="ts">
import { defineProps, defineEmits } from 'vue';
import { 
    Eye, EyeOff, Lock, Unlock, 
    Square, Circle, Image as ImageIcon, PenTool, Type, Folder, FileQuestion, 
    Hexagon, Star 
} from 'lucide-vue-next';

const props = defineProps<{
    obj: any;
    selectedIds: number[];
    depth?: number;
}>();

const emit = defineEmits<{
    (e: 'select', id: number, event: MouseEvent): void;
    (e: 'toggle-visible', id: number, visible: boolean): void;
    (e: 'toggle-lock', id: number, locked: boolean): void;
}>();

const currentDepth = props.depth || 0;
</script>

<template>
    <div 
        :class="['layer-item', { selected: selectedIds.includes(obj.id) }]"
        :style="{ paddingLeft: (8 + currentDepth * 12) + 'px' }"
        @click.stop="(e) => emit('select', obj.id, e)"
    >
        <button 
            class="visibility-toggle" 
            @click.stop="emit('toggle-visible', obj.id, !obj.visible)"
            title="Toggle Visibility"
        >
            <component :is="obj.visible ? Eye : EyeOff" :size="14" />
        </button>
        <span class="layer-icon">
            <Square v-if="obj.shape_type === 'Rectangle'" :size="14" />
            <Circle v-else-if="obj.shape_type === 'Circle'" :size="14" />
            <component :is="Circle" v-else-if="obj.shape_type === 'Ellipse'" :size="14" class="ellipse-icon" />
            <Star v-else-if="obj.shape_type === 'Star'" :size="14" />
            <Hexagon v-else-if="obj.shape_type === 'Polygon'" :size="14" />
            <ImageIcon v-else-if="obj.shape_type === 'Image'" :size="14" />
            <PenTool v-else-if="obj.shape_type === 'Path'" :size="14" />
            <Type v-else-if="obj.shape_type === 'Text'" :size="14" />
            <Folder v-else-if="obj.shape_type === 'Group'" :size="14" />
            <FileQuestion v-else :size="14" />
        </span>
        <span class="layer-name">{{ obj.name }}</span>
        <button 
            class="lock-toggle" 
            @click.stop="emit('toggle-lock', obj.id, !obj.locked)"
            title="Toggle Lock"
        >
            <component :is="obj.locked ? Lock : Unlock" :size="12" />
        </button>
    </div>
    <!-- Children -->
    <template v-if="obj.children && obj.children.length > 0">
        <LayerItem 
            v-for="child in [...obj.children].reverse()" 
            :key="child.id"
            :obj="child"
            :selectedIds="selectedIds"
            :depth="currentDepth + 1"
            @select="(id, e) => emit('select', id, e)"
            @toggle-visible="(id, v) => emit('toggle-visible', id, v)"
            @toggle-lock="(id, l) => emit('toggle-lock', id, l)"
        />
    </template>
</template>

<style scoped>
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
</style>
