import { invoke } from '@tauri-apps/api/core'
import './style.css'

interface AppConfig {
  save_path: string | null
}

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
        Manage your save files for Into the Dead: Our Darkest Days.
      </p>
    </header>

    <section class="panel">
      <h2>Active Configuration</h2>
      <div class="input-group">
        <input type="text" id="manual-path" placeholder="C:\\Path\\To\\Save\\Folder" spellcheck="false" />
        <button id="save-config" type="button">Set Path</button>
      </div>
      <p id="config-status" class="status-text"></p>
    </section>

    <section class="panel">
      <h2>Discovery</h2>
      <div class="actions">
        <button id="detect" type="button">Auto Detect Steam Paths</button>
      </div>
      <ul id="paths" class="paths" aria-live="polite"></ul>
    </section>
  </main>
`

const detectButton = document.querySelector<HTMLButtonElement>('#detect')
const pathsList = document.querySelector<HTMLUListElement>('#paths')
const manualInput = document.querySelector<HTMLInputElement>('#manual-path')
const saveButton = document.querySelector<HTMLButtonElement>('#save-config')
const configStatus = document.querySelector<HTMLParagraphElement>('#config-status')

if (!detectButton || !pathsList || !manualInput || !saveButton || !configStatus) {
  throw new Error('UI elements not found')
}

/**
 * Updates the configuration status message.
 */
function setConfigStatus(message: string, type: 'info' | 'success' | 'error' = 'info'): void {
  configStatus!.textContent = message
  configStatus!.className = 'status-text'
  if (type !== 'info') {
    configStatus!.classList.add(type)
  }
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
    item.title = 'Click to use this path'
    item.addEventListener('click', () => {
      manualInput!.value = path
      setConfigStatus('Path selected from list. Click "Set Path" to save.', 'info')
    })
    pathsList!.appendChild(item)
  }
}

/**
 * Loads the current configuration from the backend.
 */
async function loadConfig(): Promise<void> {
  try {
    const config = await invoke<AppConfig>('get_config')
    if (config.save_path) {
      manualInput!.value = config.save_path
      setConfigStatus('Configuration loaded.', 'info')
    } else {
      setConfigStatus('No save path configured.', 'info')
    }
  } catch (error) {
    console.error('Failed to load config:', error)
    setConfigStatus('Failed to load configuration.', 'error')
  }
}

/**
 * Validates and saves the user-provided path.
 */
async function savePath(): Promise<void> {
  const path = manualInput!.value.trim()
  
  if (!path) {
    setConfigStatus('Please enter a path.', 'error')
    return
  }

  setConfigStatus('Validating...', 'info')
  saveButton!.disabled = true

  try {
    const isValid = await invoke<boolean>('validate_path', { path })
    if (!isValid) {
      setConfigStatus('Path does not exist or is invalid.', 'error')
      return
    }

    await invoke('set_save_path', { path })
    setConfigStatus('Save path updated successfully.', 'success')
  } catch (error) {
    console.error('Failed to save path:', error)
    setConfigStatus('Error saving path.', 'error')
  } finally {
    saveButton!.disabled = false
  }
}

/**
 * Calls the backend command to detect Steam save paths.
 */
async function detectSteamSavePaths(): Promise<void> {
  detectButton!.disabled = true
  detectButton!.textContent = 'Scanning...'

  try {
    const paths = await invoke<string[]>('detect_steam_save_paths')
    renderPaths(paths)
    if (paths.length > 0) {
      // Optional: auto-fill if empty
      if (!manualInput!.value) {
        manualInput!.value = paths[0]
        setConfigStatus('Path detected. Click "Set Path" to save.', 'info')
      }
    }
  } catch (error) {
    console.error(error)
    pathsList!.innerHTML = '<li class="error">Detection failed</li>'
  } finally {
    detectButton!.disabled = false
    detectButton!.textContent = 'Auto Detect Steam Paths'
  }
}

// Event Listeners
detectButton!.addEventListener('click', () => {
  void detectSteamSavePaths()
})

saveButton!.addEventListener('click', () => {
  void savePath()
})

// Initial load
void loadConfig()