// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// https://astro.build/config
export default defineConfig({
  devToolbar: { enabled: false },
  integrations: [
    starlight({
      title: "canon",
      description:
        "Harness knowledge substrate: spec planning, machine-enforced completion, unified agent-session logging, and accumulation-driven harness improvement.",
      defaultLocale: "root",
      locales: {
        root: { label: "English", lang: "en" },
        ko: { label: "한국어", lang: "ko" },
      },
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/journeyWorker/canon",
        },
      ],
      customCss: ["./src/styles/theme.css"],
      expressiveCode: {
        // Code sits in the same inset log field as the landing's checks panel;
        // light mode gets GitHub's light palette so it stays readable.
        themes: ["github-dark-default", "github-light-default"],
        styleOverrides: {
          borderRadius: "10px",
          borderWidth: "1px",
          borderColor: ({ theme }) => (theme.type === "dark" ? "#30363d" : "#d1d9e0"),
          codeBackground: ({ theme }) => (theme.type === "dark" ? "#0a0d12" : "#f6f8fa"),
          codeFontFamily: "'Geist Mono Variable', ui-monospace, Menlo, monospace",
          codeFontSize: "0.875rem",
          codeLineHeight: "1.7",
          uiFontFamily: "'Mona Sans Variable', system-ui, sans-serif",
          frames: {
            frameBoxShadowCssValue: "none",
            editorTabBarBackground: ({ theme }) => (theme.type === "dark" ? "#151b23" : "#ffffff"),
            editorActiveTabBackground: ({ theme }) => (theme.type === "dark" ? "#0a0d12" : "#f6f8fa"),
            editorActiveTabIndicatorTopColor: "transparent",
            editorActiveTabIndicatorBottomColor: "transparent",
            editorTabBarBorderBottomColor: ({ theme }) => (theme.type === "dark" ? "#30363d" : "#d1d9e0"),
            terminalBackground: ({ theme }) => (theme.type === "dark" ? "#0a0d12" : "#f6f8fa"),
            terminalTitlebarBackground: ({ theme }) => (theme.type === "dark" ? "#151b23" : "#ffffff"),
            terminalTitlebarBorderBottomColor: ({ theme }) => (theme.type === "dark" ? "#30363d" : "#d1d9e0"),
            terminalTitlebarForeground: ({ theme }) => (theme.type === "dark" ? "#9198a1" : "#59636e"),
            terminalTitlebarDotsOpacity: "0",
          },
        },
      },
      sidebar: [
        {
          label: "Getting Started",
          translations: { ko: "시작하기" },
          slug: "getting-started",
        },
        {
          label: "Data & Privacy",
          translations: { ko: "데이터 & 프라이버시" },
          slug: "privacy",
        },
        {
          label: "Concepts",
          translations: { ko: "개념" },
          items: [
            { slug: "concepts/canon" },
            { slug: "concepts/trust-spine" },
            { slug: "concepts/tiered-storage" },
            { slug: "concepts/strategy-memory" },
          ],
        },
        {
          label: "Architecture",
          translations: { ko: "아키텍처" },
          slug: "architecture",
        },
        { label: "CLI", slug: "cli" },
        { label: "Examples", translations: { ko: "예제" }, slug: "examples" },
      ],
    }),
  ],
});
