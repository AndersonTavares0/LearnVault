export type Route = 'login' | 'home' | 'catalog' | 'player';

export function routeFromHash(hash: string): Route {
	const value = hash.replace(/^#\/?/, '').split('/')[0];
	if (value === 'home' || value === 'catalog' || value === 'player') return value;
	return 'login';
}

export function href(route: Route): string {
	return `#/${route}`;
}
