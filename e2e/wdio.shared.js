import fs from 'fs';
import net from 'net';
import os from 'os';
import path from 'path';
import { spawn, spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

export function createWdioConfig({
  specs,
  dataRootName,
  env = {},
  mochaTimeout = 60000,
  snapshot = {},
}) {
  const __dirname = fileURLToPath(new URL('.', import.meta.url));
  const rootDir = path.resolve(__dirname, '..');
  const isWindows = process.platform === 'win32';
  const appBinary = isWindows ? 'postium-mail.exe' : 'postium-mail';
  const tauriDriverBinary = isWindows ? 'tauri-driver.exe' : 'tauri-driver';
  const cargoBinDir = path.resolve(os.homedir(), '.cargo', 'bin');
  if (!process.env.WDIO_PORT) {
    process.env.WDIO_PORT = String(
      42000 + Math.floor(Math.random() * 10000)
    );
  }
  const wdioPort = Number(process.env.WDIO_PORT);
  const e2eDataRoot = path.resolve(rootDir, dataRootName);
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
  const e2eEnv = {
    ...process.env,
    POSTIUM_E2E: '1',
    POSTIUM_DATA_DIR: runDataDir,
    ...env,
  };

  let hasFailure = false;
  let tauriDriver;
  let exit = false;
  let closingDriver;
  let driverClosed = false;
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
    const environment = {
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
      extra: snapshot,
    };

    fs.writeFileSync(environmentLogPath, `${JSON.stringify(environment, null, 2)}\n`);
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

  async function startTauriDriver() {
    if (tauriDriver) return;

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
        env: e2eEnv,
      }
    );

    tauriDriver.stdout.pipe(process.stdout);
    tauriDriver.stderr.pipe(process.stderr);
    tauriDriver.stdout.pipe(driverLogStream, { end: false });
    tauriDriver.stderr.pipe(driverLogStream, { end: false });

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
  }

  function driverExitState() {
    return {
      exitCode: tauriDriver?.exitCode ?? null,
      signalCode: tauriDriver?.signalCode ?? null,
    };
  }

  function waitForDriverExit(timeoutMs = 5000) {
    if (!tauriDriver || tauriDriver.exitCode !== null) {
      return Promise.resolve(driverExitState());
    }

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        resolve({
          ...driverExitState(),
          timeoutMs,
        });
      }, timeoutMs);

      tauriDriver.once('exit', (code, signal) => {
        clearTimeout(timer);
        resolve({
          exitCode: code,
          signalCode: signal,
        });
      });
    });
  }

  function endDriverLogStream() {
    if (
      !driverLogStream ||
      driverLogStream.destroyed ||
      driverLogStream.writableEnded
    ) {
      return Promise.resolve();
    }

    return new Promise((resolve) => {
      driverLogStream.end(resolve);
    });
  }

  async function closeTauriDriver() {
    if (driverClosed) return;
    if (closingDriver) {
      await closingDriver;
      return;
    }

    exit = true;
    closingDriver = (async () => {
      writeDriverLog({
        event: 'close-request',
      });

      let closeResult = driverExitState();
      if (tauriDriver && tauriDriver.exitCode === null) {
        tauriDriver.kill();
        closeResult = await waitForDriverExit();
      }

      writeDriverLog({
        event: 'close-complete',
        ...closeResult,
      });
      await endDriverLogStream();
      driverClosed = true;
    })();

    await closingDriver;
  }

  function onShutdown(fn) {
    const cleanup = async () => {
      try {
        await fn();
      } finally {
        process.exit();
      }
    };
    process.once('SIGINT', cleanup);
    process.once('SIGTERM', cleanup);
    process.once('SIGHUP', cleanup);
    process.once('SIGBREAK', cleanup);
  }

  onShutdown(async () => {
    await closeTauriDriver();
    cleanupRunData();
  });

  return {
    host: '127.0.0.1',
    port: wdioPort,
    specs,
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
      timeout: mochaTimeout,
    },

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
          env: e2eEnv,
        }
      );

      if (build.status !== 0) {
        throw new Error(`Tauri debug build failed with status ${build.status}`);
      }

      if (!fs.existsSync(appPath)) {
        throw new Error(`Tauri app binary not found: ${appPath}`);
      }

      await startTauriDriver();
    },

    afterTest: async (test, context, { error }) => {
      if (!error) return;

      hasFailure = true;
      const safeTitle = test.title.replace(/[^a-z0-9-_]+/gi, '-').toLowerCase();
      await browser.saveScreenshot(path.join(screenshotsDir, `${safeTitle}.png`));
    },

    onComplete: async () => {
      await closeTauriDriver();
      cleanupRunData();
    },
  };
}
