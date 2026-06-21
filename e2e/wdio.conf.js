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
const driverLogPath = path.join(logsDir, `tauri-driver-${runId}.log`);
const environmentLogPath = path.join(logsDir, `environment-${runId}.json`);
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

function runCommand(command, args = []) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    shell: true,
  });

  return {
    command: [command, ...args].join(' '),
    status: result.status,
    stdout: result.stdout?.trim() ?? '',
    stderr: result.stderr?.trim() ?? '',
  };
}

function writeEnvironmentSnapshot() {
  const snapshot = {
    runId,
    rootDir,
    appPath,
    driverPath,
    runDataDir,
    wdioPort,
    platform: process.platform,
    arch: process.arch,
    node: process.version,
    bun: runCommand('bun', ['--version']),
    tauriDriver: runCommand(driverPath, ['--version']),
    webkitWebDriver: runCommand('WebKitWebDriver', ['--version']),
    webkitWebdriverLowercase: runCommand('webkitwebdriver', ['--version']),
    env: {
      DISPLAY: process.env.DISPLAY ?? '',
      WAYLAND_DISPLAY: process.env.WAYLAND_DISPLAY ?? '',
      XDG_SESSION_TYPE: process.env.XDG_SESSION_TYPE ?? '',
      GDK_BACKEND: process.env.GDK_BACKEND ?? '',
      WEBKIT_DISABLE_COMPOSITING_MODE: process.env.WEBKIT_DISABLE_COMPOSITING_MODE ?? '',
    },
  };

  fs.writeFileSync(environmentLogPath, `${JSON.stringify(snapshot, null, 2)}\n`);
}

function writeDriverLog(event) {
  if (
    !driverLogStream ||
    driverLogStream.destroyed ||
    driverLogStream.writableEnded
  ) {
    return;
  }

  driverLogStream.write(
    JSON.stringify({
      ...event,
      timestamp: event.timestamp ?? new Date().toISOString(),
    }) + '\n'
  );
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
    writeEnvironmentSnapshot();

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
    driverLogStream = fs.createWriteStream(driverLogPath, {
      flags: 'a',
    });

    writeDriverLog({
      event: 'start',
      runId,
      appPath,
      driverPath,
      runDataDir,
      wdioPort,
      pid: process.pid,
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
      writeDriverLog({
        event: 'error',
        message: error.message,
        stack: error.stack,
      });
      console.error('tauri-driver error:', error);
      process.exit(1);
    });

    tauriDriver.on('exit', (code, signal) => {
      writeDriverLog({
        event: 'exit',
        code,
        signal,
        expected: exit,
      });
      if (!exit) {
        hasFailure = true;
        console.error('tauri-driver exited unexpectedly:', { code, signal });
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
  writeDriverLog({
    event: 'close-request',
  });
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
