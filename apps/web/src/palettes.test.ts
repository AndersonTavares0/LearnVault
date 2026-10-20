import { describe, expect, it } from 'vitest';
import { PALETTE_PRESETS, PALETTE_STORAGE_KEY, DEFAULT_PALETTE_ID, readPalettePreferences, persistPalettePreferences, resolvePalette, validatePalette, paletteVariables, type Palette, type PalettePreferences } from './palettes';
import { persistThemeMode, readThemeMode } from './theme';
const copy = (): Palette => ({ ...structuredClone(PALETTE_PRESETS[0]), id: 'custom-test', name: 'Meu estudo' });
const prefs = (customPalettes: Palette[], selectedPaletteId = 'custom-test'): PalettePreferences => ({ version: 1, selectedPaletteId, customPalettes });
const storage = (value: unknown) => ({ getItem: () => JSON.stringify(value) });
describe('paletas locais com contraste verificável', () => {
 it('valida todos os pares dos dois modos dos presets', () => { for (const palette of PALETTE_PRESETS) expect(validatePalette(palette)).toEqual([]); });
 it('rejeita cor arbitrária, contraste insuficiente e nomes vazios', () => {
  const palette = copy(); palette.light.text = '#GG0000'; expect(validatePalette(palette).some(e => e.field.includes('text'))).toBe(true);
  palette.light.text = palette.light.background; expect(validatePalette(palette).some(e => e.field.includes('text') && e.message.includes('4,5'))).toBe(true);
  palette.name = ' '; expect(validatePalette(palette).some(e => e.field === 'name')).toBe(true);
 });
 it('retorna padrão em armazenamento ausente, bloqueado, JSON quebrado e versão desconhecida', () => {
  for (const value of [null, { getItem: () => { throw Error('bloqueado'); } }, { getItem: () => '{quebrado' }, storage({ version: 2 })]) expect(resolvePalette(readPalettePreferences(value)).id).toBe(DEFAULT_PALETTE_ID);
 });
 it('ignora custom que usa ID de preset, IDs duplicados e nomes duplicados sem distinguir caixa', () => {
  const custom = copy();
  expect(readPalettePreferences(storage(prefs([{ ...custom, id: DEFAULT_PALETTE_ID }]))).customPalettes).toEqual([]);
  const duplicate = readPalettePreferences(storage(prefs([custom, { ...copy(), name: 'Outro' }])));
  expect(duplicate.customPalettes.length).toBeLessThanOrEqual(1);
  const presetName = { ...custom, name: PALETTE_PRESETS[0].name.toUpperCase() };
  expect(readPalettePreferences(storage(prefs([presetName]))).customPalettes).toEqual([]);
 });
 it('não seleciona paleta inválida ou inexistente e não injeta propriedades livres', () => {
  const custom = copy(); custom.dark.text = 'url(https://invalid.example)';
  expect(resolvePalette(readPalettePreferences(storage(prefs([custom])))).id).toBe(DEFAULT_PALETTE_ID);
  expect(resolvePalette(prefs([], 'missing')).id).toBe(DEFAULT_PALETTE_ID);
  const variables = paletteVariables({ ...copy(), arbitrary: 'url(https://invalid.example)' } as Palette, 'dark');
  expect(Object.keys(variables).every(k => k.startsWith('--'))).toBe(true); expect(Object.values(variables).join(' ')).not.toContain('invalid.example');
 });
 it('preserva todas as 28 cores normalizadas sem perder o modo legado', () => {
  const values = new Map<string, string>(); const memory = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
  const custom = copy(); custom.light.background = custom.light.background.toLowerCase();
  persistThemeMode(memory, 'dark'); expect(persistPalettePreferences(memory, prefs([custom]))).toBe(true);
  expect(values.has(PALETTE_STORAGE_KEY)).toBe(true); const restored = resolvePalette(readPalettePreferences(memory));
  for (const mode of ['light', 'dark'] as const) for (const field of Object.keys(custom[mode]) as (keyof Palette['light'])[]) expect(restored[mode][field]).toBe(custom[mode][field].toUpperCase());
  expect(readThemeMode(memory)).toBe('dark');
 });
 it('retorna false sem storage e quando gravação falha', () => {
  expect(persistPalettePreferences(null, prefs([copy()]))).toBe(false);
  expect(persistPalettePreferences({ setItem: () => { throw Error('bloqueado'); } }, prefs([copy()]))).toBe(false);
 });
});
