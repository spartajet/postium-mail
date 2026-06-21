import fs from 'fs';
import net from 'net';
import os from 'os';
import path from 'path';
import { spawn, spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const isWindows = process.platform === 'win32';
const appBinary = isWindows ? 'postium-mail.exe' : 'postium-mail';
const tauriDriverBinary = isWindows ? 'tauri-driver.exe' : 'tauri-driver';
const cargoBinDir = path.resolve(os.homedir(), '.cargo', 'bin');
const wdioPort = Number(process.env.WDIO_PORT || 4444);
const e2eDataRoot = path.resolve(rootDir, '.e2e-data');
const runId = `run-${Date.now()}-${process.pid}`;
const runDataDir = path.join(e2eDataRoot, runId);
const artifactsDir = path.resolve(__dirname, 'artifacts');
const screenshotsDir = path.join(artifactsDir, 'screenshots');
const reportsDir = path.join(artifactsDir, 'reports');
const logsDir = path.join(artifactsDir, 'logs');
const appPath = path.resolve(rootDir, 'src-tauri', 'target', 'debug', appBinary);
const driverPath = path.resolve(cargoBinDir, tauriDriverBinary);
let hasFailure = false;

// keep track of the `tauri-driver` child process
let tauriDriver;
let exit = false;
let driverLogStream;

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function waitForPort(port, host = '127.0.0.1', timeoutMs = 15000) {
  const startedAt = Date.now();

  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const socket = net.createConnection({ port, host });

      socket.once('connect', () => {
        socket.destroy();
        resolve();
      });

      socket.once('error', () => {
        socket.destroy();
        if (Date.now() - startedAt > timeoutMs) {
          reject(new Error(`等待 tauri-driver 端口 ${host}:${port} 超时`));
          return;
        }
        setTimeout(tryConnect, 250);
      });
    };

    tryConnect();
  });
}

function cleanupRunData() {
  if (!hasFailure && fs.existsSync(runDataDir)) {
    fs.rmSync(runDataDir, { recursive: true, force: true });
  }
}

export const config = {
  host: '127.0.0.1',
  port: wdioPort,
  specs: ['./test/specs/**/*.js'],
  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        application: appPath,
      },
    },
  ],

  reporters: [
    'spec',
    [
      'junit',
      {
        outputDir: reportsDir,
        outputFileFormat: (options) => `wdio-${options.cid}.xml`,
      },
    ],
  ],

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },

  // Build the app before tests
  onPrepare: async () => {
    ensureDir(runDataDir);
    ensureDir(screenshotsDir);
    ensureDir(reportsDir);
    ensureDir(logsDir);

    const build = spawnSync(
      'bun',
      ['run', 'tauri', 'build', '--debug', '--no-bundle'],
      {
        cwd: rootDir,
        stdio: 'inherit',
        shell: true,
        env: {
          ...process.env,
          POSTIUM_E2E: '1',
          POSTIUM_DATA_DIR: runDataDir,
        },
      }
    );

    if (build.status !== 0) {
      throw new Error(`Tauri debug build failed with status ${build.status}`);
    }

    if (!fs.existsSync(appPath)) {
      throw new Error(`Tauri app binary not found: ${appPath}`);
    }
  },

  // Start tauri-driver before each session
  beforeSession: async () => {
    ensureDir(logsDir);
    driverLogStream = fs.createWriteStream(path.join(logsDir, 'tauri-driver.log'), {
      flags: 'a',
    });

    tauriDriver = spawn(
      driverPath,
      ['--port', String(wdioPort)],
      {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: {
          ...process.env,
          POSTIUM_E2E: '1',
          POSTIUM_DATA_DIR: runDataDir,
        },
      }
    );

    tauriDriver.stdout.pipe(process.stdout);
    tauriDriver.stderr.pipe(process.stderr);
    tauriDriver.stdout.pipe(driverLogStream);
    tauriDriver.stderr.pipe(driverLogStream);

    tauriDriver.on('error', (error) => {
      console.error('tauri-driver error:', error);
      process.exit(1);
    });

    tauriDriver.on('exit', (code) => {
      if (!exit) {
        console.error('tauri-driver exited with code:', code);
        process.exit(1);
      }
    });

    await waitForPort(wdioPort);
  },

  afterTest: async (test, context, { error }) => {
    if (!error) return;

    hasFailure = true;
    const safeTitle = test.title.replace(/[^a-z0-9-_]+/gi, '-').toLowerCase();
    await browser.saveScreenshot(path.join(screenshotsDir, `${safeTitle}.png`));
  },

  // Cleanup tauri-driver after session
  afterSession: () => {
    closeTauriDriver();
  },

  onComplete: () => {
    closeTauriDriver();
    cleanupRunData();
  },
};

function closeTauriDriver() {
  exit = true;
  tauriDriver?.kill();
  driverLogStream?.end();
}

function onShutdown(fn) {
  const cleanup = () => {
    try {
      fn();
    } finally {
      process.exit();
    }
  };
  process.on('exit', cleanup);
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);
  process.on('SIGHUP', cleanup);
  process.on('SIGBREAK', cleanup);
}

onShutdown(() => {
  closeTauriDriver();
  cleanupRunData();
});
