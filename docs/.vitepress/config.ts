import { defineConfig } from "vitepress";

export default defineConfig({
  title: "dots",
  description: "Declarative dotfiles across machines and environments",
  base: "/dots/",
  themeConfig: {
    search: { provider: "local" },
    socialLinks: [{ icon: "github", link: "https://github.com/phcurado/dots" }],
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
          { text: "Files", link: "/files" },
          { text: "SSH", link: "/ssh" },
          { text: "Packages", link: "/packages" },
          { text: "Services", link: "/services" },
          { text: "Docker Compose", link: "/docker-compose" },
          { text: "Commands", link: "/commands" },
          { text: "Outputs", link: "/outputs" },
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
          { text: "Changelog", link: "https://github.com/phcurado/dots/blob/main/CHANGELOG.md" },
          { text: "Release", link: "/release" },
        ],
      },
    ],
  },
});
