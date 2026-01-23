import { invoke } from '@tauri-apps/api/core'
import './style.css'

interface AppConfig {
  save_path: string | null
}

interface BackupInfo {
  path: string
  filename: string
  original_filename: string
  original_path: string
  size: number
  modified: string
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
      <h2>Backups</h2>
      <div class="actions">
        <button id="refresh-backups" type="button" disabled>Refresh Backups</button>
      </div>
      <div class="table-container">
        <table id="backups-table">
          <thead>
            <tr>
              <th>File</th>
              <th>Date</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody id="backups-list">
            <tr><td colspan="3" class="empty">No backups found.</td></tr>
          </tbody>
        </table>
      </div>
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
const refreshBackupsButton = document.querySelector<HTMLButtonElement>('#refresh-backups')
const backupsList = document.querySelector<HTMLTableSectionElement>('#backups-list')

if (!detectButton || !pathsList || !manualInput || !saveButton || !configStatus || !refreshBackupsButton || !backupsList) {
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
 * Formats a date string for display.
 */
function formatDate(isoString: string): string {
  try {
    const date = new Date(isoString)
    return date.toLocaleString()
  } catch {
    return isoString
  }
}

/**
 * Renders the list of backups.
 */
function renderBackups(backups: BackupInfo[]): void {
  backupsList!.innerHTML = ''

  if (backups.length === 0) {
    backupsList!.innerHTML = '<tr><td colspan="3" class="empty">No backups found.</td></tr>'
    return
  }

  for (const backup of backups) {
    const row = document.createElement('tr')
    
    const fileCell = document.createElement('td')
    // Display original filename for clarity, or full backup name if needed. 
    // Backup name has timestamp, so maybe just show "SaveGame" and let Date column handle time?
    // Actually, showing the filename helps confirm which file it is if watching a dir.
    fileCell.textContent = backup.original_filename
    fileCell.title = backup.filename // tooltip shows actual backup file
    
    const dateCell = document.createElement('td')
    dateCell.textContent = formatDate(backup.modified)
    
    const actionCell = document.createElement('td')
    const restoreBtn = document.createElement('button')
    restoreBtn.textContent = 'Restore'
    restoreBtn.className = 'small'
    restoreBtn.addEventListener('click', () => restoreBackup(backup))
    actionCell.appendChild(restoreBtn)

    row.appendChild(fileCell)
    row.appendChild(dateCell)
    row.appendChild(actionCell)
    
    backupsList!.appendChild(row)
  }
}

/**
 * Loads backups from the backend.
 */
async function loadBackups(): Promise<void> {
  if (!manualInput!.value) return

  refreshBackupsButton!.textContent = 'Refreshing...'
  refreshBackupsButton!.disabled = true

  try {
    const backups = await invoke<BackupInfo[]>('get_backups_command')
    renderBackups(backups)
  } catch (error) {
    console.error('Failed to load backups:', error)
    backupsList!.innerHTML = '<tr><td colspan="3" class="error">Failed to load backups</td></tr>'
  } finally {
    refreshBackupsButton!.textContent = 'Refresh Backups'
    refreshBackupsButton!.disabled = false
  }
}

/**
 * Restores a backup.
 */
async function restoreBackup(backup: BackupInfo): Promise<void> {
  let message = `Are you sure you want to restore "${backup.original_filename}" from ${formatDate(backup.modified)}?`
  message += `\nThis will overwrite the current save file.`

  try {
    const isCloud = await invoke<boolean>('check_steam_cloud_path', { path: backup.original_path })
    if (isCloud) {
      message += `\n\nWARNING: Steam Cloud folder detected.\nSteam may overwrite this restore with its cloud copy unless you launch in Offline Mode or disable Steam Cloud.`
    }
  } catch (error) {
    console.warn('Failed to check Steam Cloud status:', error)
  }

  const confirmed = window.confirm(message)
  if (!confirmed) return

  try {
    await invoke('restore_backup_command', {
      backup_path: backup.path,
      target_path: backup.original_path
    })
    alert('Restore successful!')
    // Optionally refresh backups (though they shouldn't change from a restore)
  } catch (error) {
    console.error('Restore failed:', error)
    alert(`Restore failed: ${error}`)
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
      void loadBackups() // Load backups if config exists
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
    void loadBackups() // Refresh backups after update
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

refreshBackupsButton!.addEventListener('click', () => {
  void loadBackups()
})

// Initial load
void loadConfig()