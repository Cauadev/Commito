import { createApp } from 'vue'
import { createPinia } from 'pinia'
import '@fontsource-variable/inter'
import '@fontsource-variable/jetbrains-mono'
import './app/styles.css'
import App from './App.vue'

document.documentElement.classList.add('dark')
document.documentElement.lang = 'pt-BR'

const app = createApp(App)
app.use(createPinia())
app.mount('#app')

// Disable the WebView's default context menu outside of inputs (we provide our own menus).
window.addEventListener('contextmenu', (e) => {
  const el = e.target as HTMLElement | null
  if (el && (el.closest('input, textarea, [contenteditable="true"]') || window.getSelection()?.toString())) return
  e.preventDefault()
})
