import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import { listen } from "@tauri-apps/api/event";

import { AppLayout } from "@/layouts/app-layout";
import { GateLayout } from "@/layouts/gate-layout";
import { RootLayout } from "@/layouts/root-layout";
import { LauncherPage } from "@/pages/launcher-page";
import { Settings } from "@/pages/settings";

const rootRoute = createRootRoute({
  component: RootLayout,
});

const gateRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "_gate",
  component: GateLayout,
});

const appLayoutRoute = createRoute({
  getParentRoute: () => gateRoute,
  id: "_app",
  component: AppLayout,
});

const indexRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: "/",
  component: LauncherPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => gateRoute,
  path: "/settings",
  component: Settings,
});

const hashHistory = createHashHistory();

const routeTree = rootRoute.addChildren([
  gateRoute.addChildren([appLayoutRoute.addChildren([indexRoute]), settingsRoute]),
]);

const router = createRouter({ routeTree, history: hashHistory });

// Tray menu emits "navigate" events to change routes (e.g., "Settings..." item)
listen<string>("navigate", (event) => {
  router.navigate({ to: event.payload });
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export { router };
