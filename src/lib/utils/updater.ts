import { showInfo } from '$lib/stores/notification';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { writable } from 'svelte/store';

export const updateAvailable = writable<{ available: boolean; version: string | null }>({
  available: false,
  version: null
});

export const isCheckingForUpdates = writable(false);

let cachedUpdate: Update | null = null;
let periodicTimer: ReturnType<typeof setInterval> | null = null;
let lastNotifiedVersion: string | null = null;

const MAX_BODY_CHARS = 500;
const DEFAULT_PERIODIC_INTERVAL_MS = 6 * 60 * 60 * 1000;

function formatUpdateMessage(update: Update): string {
  const released = update.date ? ` (released ${update.date.slice(0, 10)})` : '';
  const header = `A new version (${update.version}) is available${released}.`;
  const body = update.body?.trim();
  if (!body) {
    return `${header} Would you like to update now?`;
  }
  const trimmed =
    body.length > MAX_BODY_CHARS ? `${body.slice(0, MAX_BODY_CHARS).trimEnd()}…` : body;
  return `${header}\n\n${trimmed}\n\nWould you like to update now?`;
}

/**
 * Check for updates silently. Shows a toast the first time a new version is
 * detected; subsequent silent checks for the same version stay quiet.
 */
export async function checkForUpdatesSilently(): Promise<boolean> {
  isCheckingForUpdates.set(true);
  try {
    const update = await check();
    cachedUpdate = update;

    if (update?.available) {
      updateAvailable.set({ available: true, version: update.version });
      if (update.version !== lastNotifiedVersion) {
        lastNotifiedVersion = update.version;
        showInfo(`New version available: v${update.version}`);
      }
      return true;
    } else {
      updateAvailable.set({ available: false, version: null });
      return false;
    }
  } catch (error) {
    console.error('Update check failed:', error);
    return false;
  } finally {
    isCheckingForUpdates.set(false);
  }
}

/**
 * Check for updates and optionally show dialog
 */
export async function checkForUpdates(showNoUpdateDialog = false) {
  isCheckingForUpdates.set(true);
  let update: Update | null = null;
  try {
    update = await check();
    cachedUpdate = update;

    if (update?.available) {
      updateAvailable.set({ available: true, version: update.version });
      lastNotifiedVersion = update.version;
    } else {
      updateAvailable.set({ available: false, version: null });
    }
  } catch (error) {
    console.error('Update check failed:', error);
    return;
  } finally {
    isCheckingForUpdates.set(false);
  }

  if (update?.available) {
    const shouldUpdate = await ask(formatUpdateMessage(update), {
      title: 'Update Available'
    });
    if (shouldUpdate) {
      await update.downloadAndInstall();
      await relaunch();
    }
  } else if (showNoUpdateDialog) {
    await ask('You are already running the latest version.', {
      title: 'No Updates'
    });
  }
}

/**
 * Prompt to install a cached update
 */
export async function promptInstallUpdate() {
  if (!cachedUpdate?.available) {
    await checkForUpdates(false);
    return;
  }

  const shouldUpdate = await ask(formatUpdateMessage(cachedUpdate), {
    title: 'Update Available'
  });

  if (shouldUpdate) {
    await cachedUpdate.downloadAndInstall();
    await relaunch();
  }
}

/**
 * Start a periodic background check. Returns a cancel function.
 * Calling again while an interval is already running is a no-op so HMR re-mounts
 * in dev don't accumulate timers.
 */
export function startPeriodicUpdateChecks(
  intervalMs: number = DEFAULT_PERIODIC_INTERVAL_MS
): () => void {
  if (periodicTimer !== null) {
    return stopPeriodicUpdateChecks;
  }
  periodicTimer = setInterval(() => {
    if (cachedUpdate?.available) return;
    void checkForUpdatesSilently();
  }, intervalMs);
  return stopPeriodicUpdateChecks;
}

export function stopPeriodicUpdateChecks(): void {
  if (periodicTimer !== null) {
    clearInterval(periodicTimer);
    periodicTimer = null;
  }
}
