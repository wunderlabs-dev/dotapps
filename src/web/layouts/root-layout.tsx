import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Outlet } from "@tanstack/react-router";

import { AppErrorBoundary } from "@/components/app-error-boundary";
import { PlatformProvider, ToastProvider } from "@/context";

const queryClient = new QueryClient({
  defaultOptions: {
    mutations: { retry: false },
    queries: { retry: false },
  },
});

const RootLayout = () => (
  <PlatformProvider>
    <ToastProvider>
      <QueryClientProvider client={queryClient}>
        <AppErrorBoundary>
          <Outlet />
        </AppErrorBoundary>
      </QueryClientProvider>
    </ToastProvider>
  </PlatformProvider>
);

export { RootLayout };
