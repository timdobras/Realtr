import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { writable } from 'svelte/store';

// Store for update availability
export const updateAvailable = writable<{ available: boolean; version: string | null }>({
	available: false,
	version: null
});

let cachedUpdate: Update | null = null;

/**
 * Check for updates silently and store the result
 */
export async function checkForUpdatesSilently(): Promise<boolean> {
	try {
		const update = await check();
		cachedUpdate = update;

		if (update?.available) {
			updateAvailable.set({ available: true, version: update.version });
			return true;
		} else {
			updateAvailable.set({ available: false, version: null });
			return false;
		}
	} catch (error) {
		console.error('Update check failed:', error);
		return false;
	}
}

/**
 * Check for updates and optionally show dialog
 */
export async function checkForUpdates(showNoUpdateDialog = false) {
	try {
		const update = await check();
		cachedUpdate = update;

		if (update?.available) {
			updateAvailable.set({ available: true, version: update.version });

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
		} else {
			updateAvailable.set({ available: false, version: null });
			if (showNoUpdateDialog) {
				await ask('You are already running the latest version.', {
					title: 'No Updates'
				});
			}
		}
	} catch (error) {
		console.error('Update check failed:', error);
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

	const shouldUpdate = await ask(
		`A new version (${cachedUpdate.version}) is available. Would you like to update now?`,
		{
			title: 'Update Available'
		}
	);

	if (shouldUpdate) {
		await cachedUpdate.downloadAndInstall();
		await relaunch();
	}
}
