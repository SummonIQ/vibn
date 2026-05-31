'use client';

import { AnalyticsProvider, WebVitals } from '@summoniq/signalsplash-client-sdk/react';
import type { AnalyticsConfig } from '@summoniq/signalsplash-client-sdk';

interface Props {
  children: React.ReactNode;
}

const envEndpoint = process.env.NEXT_PUBLIC_ANALYTICS_ENDPOINT?.trim();
const defaultEndpoint =
  process.env.NODE_ENV === 'production'
    ? 'https://api.signalsplash.com/api/events'
    : '';
const resolvedEndpoint = envEndpoint || defaultEndpoint;
const isAnalyticsEnabled =
  process.env.NEXT_PUBLIC_ANALYTICS_ENABLED !== 'false' &&
  Boolean(resolvedEndpoint);

const analyticsConfig: AnalyticsConfig = {
  appId: 'vibn',
  endpoint: resolvedEndpoint || undefined,
  enabled: isAnalyticsEnabled,
  debug: process.env.NODE_ENV === 'development',
  trackPageViews: true,
  trackWebVitals: true,
  sessionTimeout: 30,
};

export function AppAnalyticsProvider({ children }: Props) {
  return (
    <AnalyticsProvider config={analyticsConfig}>
      <WebVitals />
      {children}
    </AnalyticsProvider>
  );
}
