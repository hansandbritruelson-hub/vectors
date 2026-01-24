import { createApp } from 'vue'
import './style.css'
import App from './App.vue'
import init from './pkg/engine'

init().then(() => {
    createApp(App).mount('#app')
}).catch(err => {
    console.error("Failed to initialize WASM engine:", err);
    // Still mount the app so we can show an error state if needed, 
    // but the engine will be missing.
    createApp(App).mount('#app')
})
