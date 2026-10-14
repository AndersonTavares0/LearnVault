import adapter from '@sveltejs/adapter-static';

export default {
	kit: {
		adapter: adapter({ fallback: '200.html', pages: 'web/build', assets: 'web/build' })
	}
};
