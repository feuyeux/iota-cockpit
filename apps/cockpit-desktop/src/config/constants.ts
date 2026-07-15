export const APP_CONFIG = {
  // Event and trace limits
  MAX_EVENTS: 300,
  MAX_TOOL_CALLS: 100,
  MAX_ACTION_RESULTS: 100,
  MAX_HUMAN_TURNS: 300,

  // Network timeouts (milliseconds)
  CONNECT_TIMEOUT: 500,
  READ_TIMEOUT: 2000,
  RECONNECT_BASE_DELAY: 500,
  RECONNECT_MAX_DELAY: 8000,
  RECONNECT_MAX_ATTEMPTS: 5,

  // Pagination
  EVENTS_PER_PAGE: 50,
  TRACES_PER_PAGE: 25,

  // LocalStorage keys
  STORAGE_KEY_LAST_SCENARIO: "cockpit:lastScenario",
  STORAGE_KEY_LAST_RUN: "cockpit:lastRun",
  STORAGE_KEY_APPROVAL_MODE: "cockpit:approvalRequired",

  // UI
  DEFAULT_SCENARIO_PATH: "scenarios/smoke-in-cockpit.yaml",
  AUTO_RUN_EVENT_POLL_INTERVAL_MS: 250,
} as const;

export const KEYBOARD_SHORTCUTS = {
  PAUSE: " ",
  STEP: "s",
  HELP: "?",
} as const;
