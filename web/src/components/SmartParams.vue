<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
    metadata: {
        id: string;
        name: string;
        parameters: {
            name: string;
            key: string;
            min: number;
            max: number;
            default: number;
            step: number;
            kind: 'range' | 'color' | 'bool';
        }[];
    };
    values: number[];
}>();

const emit = defineEmits<{
    (e: 'update', idx: number, val: number, saveUndo: boolean): void;
}>();

function onInput(idx: number, e: Event) {
    const target = e.target as HTMLInputElement;
    let val: number;
    if (target.type === 'checkbox') {
        val = target.checked ? 1 : 0;
    } else if (target.type === 'color') {
        val = parseInt(target.value.substring(1), 16);
    } else {
        val = Number(target.value);
    }
    emit('update', idx, val, false);
}

function onChange(idx: number, e: Event) {
    const target = e.target as HTMLInputElement;
    let val: number;
    if (target.type === 'checkbox') {
        val = target.checked ? 1 : 0;
    } else if (target.type === 'color') {
        val = parseInt(target.value.substring(1), 16);
    } else {
        val = Number(target.value);
    }
    emit('update', idx, val, true);
}

function numberToHex(n: number) {
    return '#' + Math.round(n).toString(16).padStart(6, '0');
}
</script>

<template>
    <div class="smart-params">
        <div class="separator-text">{{ metadata.name.toUpperCase() }} PARAMETERS</div>
        <div v-for="(p, idx) in metadata.parameters" :key="p.key" class="prop-row">
            <label>{{ p.name }}</label>
            
            <div v-if="p.kind === 'range'" class="range-with-value">
                <input type="range" 
                    :min="p.min" 
                    :max="p.max" 
                    :step="p.step" 
                    :value="values[idx]"
                    @input="onInput(idx, $event)"
                    @change="onChange(idx, $event)" />
                <span class="range-val">{{ values[idx].toFixed(2) }}</span>
            </div>

            <div v-else-if="p.kind === 'color'" class="color-row">
                <input type="color"
                    :value="numberToHex(values[idx])"
                    @input="onInput(idx, $event)"
                    @change="onChange(idx, $event)" />
                <span class="range-val">{{ numberToHex(values[idx]).toUpperCase() }}</span>
            </div>

            <div v-else-if="p.kind === 'bool'" class="bool-row">
                <input type="checkbox"
                    :checked="values[idx] > 0.5"
                    @change="onChange(idx, $event)" />
            </div>
        </div>
    </div>
</template>

<style scoped>
.smart-params {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 0;
}

.prop-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.prop-row label {
    font-size: 11px;
    color: #888;
    text-transform: uppercase;
}

.range-with-value, .color-row, .bool-row {
    display: flex;
    align-items: center;
    gap: 8px;
}

.range-with-value input {
    flex: 1;
}

.color-row input {
    width: 32px;
    height: 20px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
}

.range-val {
    font-size: 11px;
    font-family: monospace;
    min-width: 35px;
    text-align: right;
    color: #ccc;
}

.separator-text {
  font-size: 10px;
  font-weight: bold;
  color: #555;
  margin-top: 12px;
  margin-bottom: 4px;
  letter-spacing: 0.05em;
}
</style>
