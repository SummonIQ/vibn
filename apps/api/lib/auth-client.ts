import { createAuthClient } from "better-auth/client";

// Reference client for the Tauri desktop app. The actual client lives in
// the vibn Tauri repo — this is included here so the API's auth surface can
// be type-checked end-to-end if it's ever consumed from this package.
export const authClient = createAuthClient({
  baseURL: process.env.BETTER_AUTH_URL
});
