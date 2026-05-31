import type { Metadata } from "next";
import { CliHero } from "./_components/cli-hero";
import { Install } from "./_components/install";
import { SlashCommands } from "./_components/slash-commands";
import { CliFeatures } from "./_components/cli-features";

export const metadata: Metadata = {
  title: "Vibn CLI — A terminal-native local AI coding agent",
  description:
    "Vibn CLI is a Rust-built TUI coding agent. Clap CLI, Ratatui fullscreen, MCP, slash commands, built-in tools. Runs entirely on your machine.",
};

export default function CliPage() {
  return (
    <>
      <CliHero />
      <Install />
      <CliFeatures />
      <SlashCommands />
    </>
  );
}
