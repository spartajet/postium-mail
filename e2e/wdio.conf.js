import os from 'os';
import path from 'path';
import { spawn, spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const isWindows = process.platform === 'win32';
const appBinary = isWindows ? 'postium-mail.exe' : 'postium-mail';
const tauriDriverBinary = isWindows ? 'tauri-driver.exe' : 'tauri-driver';
const cargoBinDir = path.resolve(os.homedir(), '.cargo', 'bin');

// keep track of the `tauri-driver` child process
let tauriDriver;
let exit = false;

export const config = {
  host: '127.0.0.1',
  port: 4444,
  specs: ['./test/specs/**/*.js'],
  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        application: path.resolve(__dirname, '..', 'src-tauri', 'target', 'debug', appBinary),
      },
    },
  ],

  reporters: ['spec'],

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },

  // Build the app before tests
  onPrepare: () => {
    const build = spawnSync(
      'bun',
      ['run', 'tauri', 'build', '--debug', '--no-bundle'],
      {
        cwd: path.resolve(__dirname, '..'),
        stdio: 'inherit',
        shell: true,
      }
    );

    if (build.status !== 0) {
      throw new Error(`Tauri debug build failed with status ${build.status}`);
    }
  },

  // Start tauri-driver before each session
  beforeSession: () => {
    tauriDriver = spawn(
      path.resolve(cargoBinDir, tauriDriverBinary),
      [],
      { stdio: [null, process.stdout, process.stderr] }
    );

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
  },

  // Cleanup tauri-driver after session
  afterSession: () => {
    closeTauriDriver();
  },
};

function closeTauriDriver() {
  exit = true;
  tauriDriver?.kill();
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
});
