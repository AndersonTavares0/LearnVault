/** Minimal observable value for state shared outside React (timers, sync queues). */
export function createStore<T>(initial: T) {
	let value = initial;
	const listeners = new Set<() => void>();
	return {
		get: () => value,
		set(next: T) {
			value = next;
			for (const listener of listeners) listener();
		},
		update(change: (current: T) => T) {
			this.set(change(value));
		},
		subscribe(listener: () => void) {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		}
	};
}
