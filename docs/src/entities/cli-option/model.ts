/** CLI command groups represented in the configuration reference. */
export type CliCommand = "shared" | "scan" | "plan" | "review" | "apply" | "extract" | "ai-review";

/** One documented command-line option. */
export type CliOption = {
  /** Command group that owns the option. */
  command: CliCommand;
  /** Option name as shown in CLI help. */
  name: string;
  /** Option value placeholder when applicable. */
  value?: string;
  /** User-facing default value. */
  defaultValue: string;
  /** Concise behavior description. */
  description: string;
};

/** Configuration reference section. */
export type CliOptionSection = {
  /** Stable HTML anchor. */
  id: string;
  /** Section heading. */
  title: string;
  /** Optional context displayed above the option table. */
  description?: string;
  /** Options shown in the section. */
  options: CliOption[];
};
