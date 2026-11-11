import { vi } from 'vitest';

/** A stand-in for pdf.js: jsdom has no canvas, so these tests cover the reader's own behavior. */
export function fakePdfjs(numPages = 5) {
	const page = {
		getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale, scale }),
		render: () => ({ promise: Promise.resolve(), cancel: vi.fn() }),
		streamTextContent: () => ({})
	};
	// Page 3 mentions an owl, so search tests have something to find.
	const doc = { numPages, getPage: async (n: number) => ({ ...page, getTextContent: async () => ({ items: [{ str: n === 3 ? 'The owl hunts at night' : `Plain page ${n}` }] }) }) };
	return {
		getDocument: vi.fn(() => ({ promise: Promise.resolve(doc), destroy: async () => undefined })),
		GlobalWorkerOptions: {} as { workerSrc?: string },
		TextLayer: class { async render() { /* nothing to lay out in jsdom */ } }
	};
}
