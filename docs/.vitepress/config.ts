import { defineConfig } from "vitepress";

export default defineConfig({
  title: "dots",
  description: "Declarative dotfiles across machines and environments",
  base: "/dots/",
  themeConfig: {
    search: { provider: "local" },
    sidebar: [
      {
        text: "Getting started",
        items: [
          { text: "Quick start", link: "/quick-start" },
          { text: "Install", link: "/install" },
        ],
      },
      {
        text: "Managing a machine",
        items: [
          { text: "Symlinks", link: "/symlinks" },
          { text: "Packages", link: "/packages" },
          { text: "Services", link: "/services" },
          { text: "Commands", link: "/commands" },
          { text: "Fonts", link: "/fonts" },
          { text: "User", link: "/user" },
        ],
      },
      {
        text: "Multiple machines",
        items: [
          { text: "Platforms and profiles", link: "/platforms-and-profiles" },
          { text: "Organization", link: "/organization" },
          { text: "State", link: "/state" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Release", link: "/release" },
        ],
      },
    ],
  },
});
