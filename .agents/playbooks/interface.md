# Interface change playbook

1. Read the roadmap and preserve LearnVault's original visual identity.
2. Ensure all user-facing text, notifications, error fallbacks, and labels use `tr(en, ptBr)` from `apps/web-next/src/lib/i18n.ts` (English default, Brazilian Portuguese fully supported).
3. Define the user-visible state changes, including loading states, empty states, error banners, keyboard focus indicators, ARIA live regions, and reduced-motion behavior.
4. Add or update the smallest relevant test in Vitest or Playwright E2E.
5. Run `pnpm run check`, `pnpm --filter learnvault-web-next test`, and `pnpm run build`.
6. Include a concise visual description or screenshots in the pull request.
