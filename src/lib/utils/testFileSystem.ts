// src/lib/utils/testFileSystem.js
import { mkdir, exists, remove } from "@tauri-apps/plugin-fs";
import { appDataDir } from "@tauri-apps/api/path";

export async function testFileSystemAccess() {
	try {
		const appDataPath = await appDataDir();
		const testPath = `${appDataPath}/test-folder`;

		console.log("🔍 Creating test folder at:", testPath);

		await mkdir(testPath, { recursive: true });
		console.log("✅ Directory creation successful");

		const existsResult = await exists(testPath);
		console.log("✅ Directory exists:", existsResult);

		await remove(testPath, { recursive: true });
		console.log("✅ Directory removal successful");

		return true;
	} catch (error) {
		console.error("❌ Filesystem test failed:", error);
		return false;
	}
}
