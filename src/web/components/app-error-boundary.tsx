import { Component, type ErrorInfo, type ReactNode } from "react";

import { AppErrorFallback } from "./app-error-fallback";

interface AppErrorBoundaryProps {
  readonly children: ReactNode;
}

interface AppErrorBoundaryState {
  readonly error: Error | null;
}

class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // The Rust panic hook covers the host process; React errors are surfaced
    // here so the diagnostics zip's host log captures them via the webview
    // console bridge.
    console.error("React error boundary caught:", error, info.componentStack);
  }

  handleReset = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error === null) {
      return this.props.children;
    }
    return <AppErrorFallback error={this.state.error} onReset={this.handleReset} />;
  }
}

export { AppErrorBoundary };
