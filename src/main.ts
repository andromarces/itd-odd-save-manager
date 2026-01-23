import { invoke } from '@tauri-apps/api/core'
import './style.css'

const app = document.querySelector<HTMLDivElement>('#app')
if (!app) {
  throw new Error('App container not found')
}

app.innerHTML = `
  <main class="layout">
    <header class="header">
      <p class="kicker">Steam on Windows</p>
      <h1>ITD ODD Save Manager</h1>
      <p class="subhead">
        Auto detect the save folder for Into the Dead: Our Darkest Days.
      </p>
    </header>
    <section class="panel">
      <div class="actions">
        <button id="detect" type="button">Detect Steam Save Paths</button>
        <span id="status" class="status" aria-live="polite">Idle</span>
      </div>
      <ul id="paths" class="paths" aria-live="polite"></ul>
    </section>
  </main>
`

const detectButton = document.querySelector<HTMLButtonElement>('#detect')
const statusLine = document.querySelector<HTMLSpanElement>('#status')
const pathsList = document.querySelector<HTMLUListElement>('#paths')

if (!detectButton || !statusLine || !pathsList) {
  throw new Error('UI elements not found')
}

/**
 * Updates the status line in the UI.
 */
function setStatus(message: string): void {
  statusLine!.textContent = message
}

/**
 * Renders the detected save paths into the list element.
 */
function renderPaths(paths: string[]): void {
  pathsList!.innerHTML = ''

  if (paths.length === 0) {
    const item = document.createElement('li')
    item.textContent = 'No Steam save paths detected.'
    item.classList.add('empty')
    pathsList!.appendChild(item)
    return
  }

  for (const path of paths) {
    const item = document.createElement('li')
    item.textContent = path
    pathsList!.appendChild(item)
  }
}

/**
 * Calls the backend command to detect Steam save paths.
 */
async function detectSteamSavePaths(): Promise<void> {
  setStatus('Scanning')
  detectButton!.disabled = true

  try {
    const paths = await invoke<string[]>('detect_steam_save_paths')
    renderPaths(paths)
    setStatus(`Found ${paths.length} path(s)`)
  } catch (error) {
    console.error(error)
    setStatus('Detection failed')
  } finally {
    detectButton!.disabled = false
  }
}

detectButton!.addEventListener('click', () => {
  void detectSteamSavePaths()
})
