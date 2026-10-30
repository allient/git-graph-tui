// @ts-check
import starlight from '@astrojs/starlight';
import { defineConfig } from 'astro/config';

const repo = 'https://github.com/allient/git-graph-tui';

// https://astro.build/config
export default defineConfig({
	// Set SITE_URL when deploying (for example https://git-graph-tui-docs.<subdomain>.workers.dev)
	// so the sitemap and canonical links use the real address.
	site: process.env.SITE_URL,
	integrations: [
		starlight({
			title: 'git-graph-tui',
			description:
				'A Git Graph for the terminal: a colored commit graph with right-click menus for checkout, merge, rebase, cherry-pick, reset and more.',
			logo: { src: './src/assets/logo.svg', replacesTitle: false },
			favicon: '/favicon.svg',
			social: [{ icon: 'github', label: 'GitHub', href: repo }],
			editLink: { baseUrl: `${repo}/edit/main/apps/docs/` },
			customCss: ['./src/styles/theme.css'],
			lastUpdated: true,
			sidebar: [
				{
					label: 'Getting started',
					items: ['getting-started/installation', 'getting-started/quick-start'],
				},
				{
					label: 'Guides',
					items: [
						'guides/graph',
						'guides/context-menus',
						'guides/commit-details',
						'guides/source-control',
					],
				},
				{
					label: 'Reference',
					items: ['reference/keys', 'reference/configuration'],
				},
				{ label: 'Integrations', items: ['integrations/herdr'] },
				{
					label: 'Contributing',
					items: ['contributing/development', 'contributing/deploying-docs'],
				},
			],
		}),
	],
});
