// ITD ODD Save Manager by andromarces

import "./style.css";
import { createBackupsFeature } from "./ui/backups";
import { createConfigFeature } from "./ui/config";
import { renderAppShell, setupTabNavigation } from "./ui/dom";
import { setupSettingsFeature } from "./ui/settings";
import { initWatcherAfterPaint } from "./ui/watcher";

const elements = renderAppShell();
setupTabNavigation(elements);
const backupsFeature = createBackupsFeature(elements);
const configFeature = createConfigFeature(elements, {
  loadBackups: backupsFeature.loadBackups,
  setRefreshAvailability: backupsFeature.setRefreshAvailability,
});

const settingsFeature = setupSettingsFeature(elements);

// Initial load
void configFeature.applyAutoDetectionAvailability();
void configFeature.loadConfig();

// Initialize watcher strictly after UI is shown (post-paint)
initWatcherAfterPaint();

// Cleanup
window.addEventListener("beforeunload", () => {
  backupsFeature.destroy();
  configFeature.destroy();
  settingsFeature.destroy();
});
