import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { getMagickBinary, getMagickEnv, resetMagickPathCache } from '../main/pipeline/magick-path.js';

/** Builds the directory layout scripts/bundle-magick-mac.sh produces. */
function makeMacBundle(root: string): void {
  fs.mkdirSync(path.join(root, 'bin'), { recursive: true });
  fs.writeFileSync(path.join(root, 'bin', 'magick'), '');
  fs.mkdirSync(path.join(root, 'lib', 'ImageMagick', 'modules-Q16HDRI', 'coders'), { recursive: true });
  fs.mkdirSync(path.join(root, 'lib', 'ImageMagick', 'modules-Q16HDRI', 'filters'), { recursive: true });
  fs.mkdirSync(path.join(root, 'lib', 'ImageMagick', 'config-Q16HDRI'), { recursive: true });
  fs.mkdirSync(path.join(root, 'etc', 'ImageMagick-7'), { recursive: true });
  fs.mkdirSync(path.join(root, 'share', 'ImageMagick-7'), { recursive: true });
}

function setPlatform(value: NodeJS.Platform): void {
  Object.defineProperty(process, 'platform', { value, configurable: true });
}

describe('magick-path', () => {
  const realPlatform = process.platform;
  const realAppRoot: string | undefined = process.env.APP_ROOT;
  let tmp: string;

  beforeEach(() => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'bite-magick-'));
    resetMagickPathCache();
  });

  afterEach(() => {
    setPlatform(realPlatform);
    // APP_ROOT and resourcesPath are declared non-optional in electron-env.d.ts,
    // so restore them by assignment rather than delete.
    process.env.APP_ROOT = realAppRoot ?? '';
    (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = '';
    fs.rmSync(tmp, { recursive: true, force: true });
    resetMagickPathCache();
  });

  it('falls back to PATH with no bundle present', () => {
    setPlatform('darwin');
    process.env.APP_ROOT = tmp;

    expect(getMagickBinary()).toBe('magick');
    expect(getMagickEnv()).toEqual({});
  });

  it('resolves the macOS bundle from the packaged app resources', () => {
    setPlatform('darwin');
    const resources = path.join(tmp, 'Resources');
    makeMacBundle(path.join(resources, 'magick'));
    (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath = resources;

    expect(getMagickBinary()).toBe(path.join(resources, 'magick', 'bin', 'magick'));
  });

  it('points the macOS bundle at its own modules and config', () => {
    setPlatform('darwin');
    const root = path.join(tmp, 'resources', 'mac', 'magick');
    makeMacBundle(root);
    process.env.APP_ROOT = tmp;

    const env = getMagickEnv();
    expect(env.MAGICK_HOME).toBe(root);
    expect(env.MAGICK_CODER_MODULE_PATH).toBe(path.join(root, 'lib', 'ImageMagick', 'modules-Q16HDRI', 'coders'));
    expect(env.MAGICK_FILTER_MODULE_PATH).toBe(path.join(root, 'lib', 'ImageMagick', 'modules-Q16HDRI', 'filters'));
    // config-Q16HDRI holds configure.xml and must be searched before etc/ and share/.
    expect(env.MAGICK_CONFIGURE_PATH?.split(path.delimiter)).toEqual([
      path.join(root, 'lib', 'ImageMagick', 'config-Q16HDRI'),
      path.join(root, 'etc', 'ImageMagick-7'),
      path.join(root, 'share', 'ImageMagick-7'),
    ]);
  });

  it('needs no environment for the Windows bundle', () => {
    setPlatform('win32');
    const root = path.join(tmp, 'resources', 'win', 'magick');
    fs.mkdirSync(root, { recursive: true });
    fs.writeFileSync(path.join(root, 'magick.exe'), '');
    process.env.APP_ROOT = tmp;

    expect(getMagickBinary()).toBe(path.join(root, 'magick.exe'));
    expect(getMagickEnv()).toEqual({});
  });

  it('memoizes the resolution', () => {
    setPlatform('darwin');
    const root = path.join(tmp, 'resources', 'mac', 'magick');
    makeMacBundle(root);
    process.env.APP_ROOT = tmp;

    const first = getMagickBinary();
    fs.rmSync(root, { recursive: true, force: true });
    expect(getMagickBinary()).toBe(first);
  });
});
