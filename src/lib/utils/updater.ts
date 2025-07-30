import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';

export async function checkForUpdates(showNoUpdateDialog = false) {
  try {
    const update = await check();

    if (update?.available) {
      const shouldUpdate = await ask(
        `A new version (${update.version}) is available. Would you like to update now?`,
        {
          title: 'Update Available'
        }
      );

      if (shouldUpdate) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } else if (showNoUpdateDialog) {
      await ask('You are already running the latest version.', {
        title: 'No Updates'
      });
    }
  } catch (error) {
    console.error('Update check failed:', error);
  }
}
