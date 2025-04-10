import { defineConfig } from "vitepress";
import pkg from "../package.json";
import markdownItFootnote from "markdown-it-footnote";
import {
  groupIconMdPlugin,
  groupIconVitePlugin,
  localIconLoader,
} from "vitepress-plugin-group-icons";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "wire",
  description: "a tool to deploy nixos systems",
  themeConfig: {
    search: {
      provider: "local",
    },

    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: "Home", link: "/" },
      { text: "Guide", link: "/guide/wire" },
      { text: "Reference", link: "/reference/cli" },
      {
        text: pkg.version,
        items: [
          {
            text: "Changelog",
            link: "https://github.com/wires-org/wire/blob/main/CHANGELOG.md",
          },
        ],
      },
    ],

    sidebar: {
      "/guide/": [
        {
          text: "Introduction",
          items: [
            { text: "What is Wire?", link: "/guide/wire" },
            { text: "Getting Started", link: "/guide/getting-started" },
            { text: "Targeting Nodes", link: "/guide/targeting" },
          ],
        },
        {
          text: "Features",
          items: [
            { text: "Secret management", link: "/guide/keys" },
            { text: "Parallelism", link: "/guide/parallelism" },
            { text: "hive.default", link: "/guide/hive-default" },
            { text: "Magic Rollback", link: "/guide/magic-rollback" },
          ],
        },
        {
          text: "Use cases",
          items: [{ text: "Tailscale", link: "/guide/tailscale" }],
        },
      ],
      "/reference/": [
        {
          text: "Reference",
          items: [
            { text: "CLI", link: "/reference/cli" },
            { text: "Module Options", link: "/reference/module" },
          ],
        },
      ],
    },

    editLink: {
      pattern: "https://github.com/wires-org/wire/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },

    socialLinks: [
      { icon: "github", link: "https://github.com/wires-org/wire" },
    ],
  },
  markdown: {
    config: (md) => {
      md.use(markdownItFootnote);
      md.use(groupIconMdPlugin);
    },
  },
  vite: {
    plugins: [
      groupIconVitePlugin({
        customIcon: {
          nixos: "vscode-icons:file-type-nix",
          "configuration.nix": "vscode-icons:file-type-nix",
          "hive.nix": "vscode-icons:file-type-nix",
          "module.nix": "vscode-icons:file-type-nix",
          home: localIconLoader(import.meta.url, "../assets/homemanager.svg"),
          ".conf": "vscode-icons:file-type-config",
        },
      }),
    ],
  },
});
