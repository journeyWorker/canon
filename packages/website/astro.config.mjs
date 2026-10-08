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
        // Code is a black field in both themes, like the landing's command bar.
        themes: ["min-dark"],
        styleOverrides: {
          borderRadius: "0",
          borderWidth: "0",
          codeBackground: "#0b0b0b",
          codeFontFamily: "'Inconsolata Variable', ui-monospace, Menlo, monospace",
          codeFontSize: "0.9375rem",
          uiFontFamily: "'Spline Sans Variable', system-ui, sans-serif",
          frames: {
            frameBoxShadowCssValue: "none",
            editorBackground: "#0b0b0b",
            editorTabBarBackground: "#0b0b0b",
            editorActiveTabBackground: "#0b0b0b",
            editorActiveTabIndicatorTopColor: "#ff4a1c",
            editorActiveTabIndicatorBottomColor: "transparent",
            editorTabBarBorderBottomColor: "#2a2a28",
            editorTabBorderRadius: "0",
            terminalBackground: "#0b0b0b",
            terminalTitlebarBackground: "#0b0b0b",
            terminalTitlebarBorderBottomColor: "#2a2a28",
            terminalTitlebarForeground: "#a3a39b",
            terminalTitlebarDotsOpacity: "0",
            inlineButtonBorder: "#a3a39b",
            tooltipSuccessBackground: "#ff4a1c",
            tooltipSuccessForeground: "#0b0b0b",
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
