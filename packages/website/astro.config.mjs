// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// https://astro.build/config
export default defineConfig({
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
