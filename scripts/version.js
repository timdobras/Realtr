#!/usr/bin/env node

/**
 * Version management script for Realtr
 *
 * Version format: {major}.{minor}.{commits}[-prerelease]
 * Example: 0.1.34-beta
 *
 * Usage:
 *   node scripts/version.js sync [--next]     - Sync patch to commit count (--next adds 1)
 *   node scripts/version.js minor             - Bump minor version
 *   node scripts/version.js major             - Bump major version
 *   node scripts/version.js prerelease <tag>  - Set prerelease tag (beta/alpha/rc)
 *   node scripts/version.js release           - Remove prerelease tag
 *   node scripts/version.js set <major> <minor> [prerelease] - Set specific major.minor
 *   node scripts/version.js tag               - Create and push git tag to trigger release
 */

import { execSync } from 'child_process';
import { readFileSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

// File paths
const PACKAGE_JSON = join(ROOT, 'package.json');
const TAURI_CONF = join(ROOT, 'src-tauri', 'tauri.conf.json');
const CARGO_TOML = join(ROOT, 'src-tauri', 'Cargo.toml');

/**
 * Get the total number of git commits
 */
function getCommitCount() {
	try {
		const count = execSync('git rev-list --count HEAD', { encoding: 'utf-8' }).trim();
		return parseInt(count, 10);
	} catch {
		console.error('Failed to get git commit count. Are you in a git repository?');
		process.exit(1);
	}
}

/**
 * Parse a semver version string
 */
function parseVersion(version) {
	const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/);
	if (!match) {
		throw new Error(`Invalid version format: ${version}`);
	}
	return {
		major: parseInt(match[1], 10),
		minor: parseInt(match[2], 10),
		patch: parseInt(match[3], 10),
		prerelease: match[4] || null
	};
}

/**
 * Format version object to string
 */
function formatVersion({ major, minor, patch, prerelease }) {
	const base = `${major}.${minor}.${patch}`;
	return prerelease ? `${base}-${prerelease}` : base;
}

/**
 * Read current version from package.json
 */
function getCurrentVersion() {
	const pkg = JSON.parse(readFileSync(PACKAGE_JSON, 'utf-8'));
	return parseVersion(pkg.version);
}

/**
 * Update package.json version
 */
function updatePackageJson(version) {
	const pkg = JSON.parse(readFileSync(PACKAGE_JSON, 'utf-8'));
	pkg.version = version;
	writeFileSync(PACKAGE_JSON, JSON.stringify(pkg, null, '\t') + '\n');
}

/**
 * Update tauri.conf.json version
 */
function updateTauriConf(version) {
	const conf = JSON.parse(readFileSync(TAURI_CONF, 'utf-8'));
	conf.version = version;
	writeFileSync(TAURI_CONF, JSON.stringify(conf, null, '\t') + '\n');
}

/**
 * Update Cargo.toml version
 */
function updateCargoToml(version) {
	let content = readFileSync(CARGO_TOML, 'utf-8');
	content = content.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
	writeFileSync(CARGO_TOML, content);
}

/**
 * Update all version files
 */
function updateAllVersions(version) {
	const versionStr = formatVersion(version);
	updatePackageJson(versionStr);
	updateTauriConf(versionStr);
	updateCargoToml(versionStr);
	console.log(`Version: ${versionStr}`);
}

// Parse command line arguments
const args = process.argv.slice(2);
const command = args[0];

if (!command) {
	console.log('Usage: node scripts/version.js <command> [options]');
	console.log('Commands: sync, minor, major, prerelease <tag>, release');
	process.exit(1);
}

const current = getCurrentVersion();
const commits = getCommitCount();

switch (command) {
	case 'sync': {
		const next = args.includes('--next');
		const newVersion = {
			...current,
			patch: next ? commits + 1 : commits
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'minor': {
		const newVersion = {
			...current,
			minor: current.minor + 1,
			patch: commits
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'major': {
		const newVersion = {
			major: current.major + 1,
			minor: 0,
			patch: commits,
			prerelease: null
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'prerelease': {
		const tag = args[1];
		if (!tag) {
			console.error('Usage: node scripts/version.js prerelease <tag>');
			console.error('Example: node scripts/version.js prerelease beta');
			process.exit(1);
		}
		const newVersion = {
			...current,
			patch: commits,
			prerelease: tag
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'release': {
		const newVersion = {
			...current,
			patch: commits,
			prerelease: null
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'set': {
		const major = parseInt(args[1], 10);
		const minor = parseInt(args[2], 10);
		const prerelease = args[3] || null;

		if (isNaN(major) || isNaN(minor)) {
			console.error('Usage: node scripts/version.js set <major> <minor> [prerelease]');
			console.error('Example: node scripts/version.js set 0 1 beta');
			process.exit(1);
		}

		const newVersion = {
			major,
			minor,
			patch: commits,
			prerelease
		};
		updateAllVersions(newVersion);
		break;
	}

	case 'tag': {
		const versionStr = formatVersion(current);
		const tagName = `v${versionStr}`;

		try {
			// Check if tag already exists
			try {
				execSync(`git rev-parse ${tagName}`, { encoding: 'utf-8', stdio: 'pipe' });
				console.error(`Tag ${tagName} already exists. Commit new changes first.`);
				process.exit(1);
			} catch {
				// Tag doesn't exist, we can create it
			}

			// Create and push the tag
			console.log(`Creating tag: ${tagName}`);
			execSync(`git tag ${tagName}`, { encoding: 'utf-8', stdio: 'inherit' });

			console.log(`Pushing tag to origin...`);
			execSync(`git push origin ${tagName}`, { encoding: 'utf-8', stdio: 'inherit' });

			console.log(`\nRelease triggered! GitHub Actions will build and publish ${tagName}`);
			console.log(`Check progress at: https://github.com/timdobras/Realtr/actions`);
		} catch (err) {
			console.error('Failed to create/push tag:', err.message);
			process.exit(1);
		}
		break;
	}

	default:
		console.error(`Unknown command: ${command}`);
		console.log(
			'Commands: sync, minor, major, prerelease <tag>, release, set <major> <minor> [prerelease], tag'
		);
		process.exit(1);
}
