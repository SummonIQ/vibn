import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { VibnLogo } from './vibn-logo';
import { Tabs } from './ui/tabs';
import { cn } from '../lib/utils';
import { api } from '../api';
import type { UserProfile } from '../types';

export interface AuthScreenProps {
  initialProfile: UserProfile;
  rememberedPassword: string | null;
  onAuthenticated: (profile: UserProfile) => void;
  currentTheme?: { from: string; to: string; light: string };
}

export function AuthScreen({
  initialProfile,
  rememberedPassword,
  onAuthenticated,
  currentTheme = { from: '#a78bfa', to: '#7c3aed', light: '#c4b5fd' },
}: AuthScreenProps) {
  const [mode, setMode] = useState<'sign-in' | 'sign-up'>('sign-in');
  const [email, setEmail] = useState(initialProfile.email || '');
  const [password, setPassword] = useState(rememberedPassword || '');
  const [firstName, setFirstName] = useState('');
  const [lastName, setLastName] = useState('');
  const [remember, setRemember] = useState(true);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const out = await api.signIn(email, password, remember);
      onAuthenticated(out.profile);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err?.message || 'Failed to sign in');
      setLoading(false);
    }
  };

  const handleSignUp = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const out = await api.signUp(email, password, firstName, lastName, remember);
      onAuthenticated(out.profile);
    } catch (err: any) {
      setError(typeof err === 'string' ? err : err?.message || 'Failed to create account');
      setLoading(false);
    }
  };

  const inputStyle: React.CSSProperties = {
    background: 'rgb(30, 30, 34)',
    borderTop: '1px solid rgba(0,0,0,0.5)',
    borderBottom: '1px solid rgba(255,255,255,0.06)',
    borderLeft: '1px solid rgba(255,255,255,0.04)',
    borderRight: '1px solid rgba(255,255,255,0.04)',
  };
  const inputClassName =
    'w-full px-2.5 py-1.5 text-[13px] rounded-md text-white placeholder-neutral-600 transition-all duration-200 focus:outline-none focus:ring-0 no-drag';

  const handleSkip = async () => {
    const profile = await api.saveUserProfile({
      display_name: email.split('@')[0] || 'guest',
      email: email || '',
      auth_endpoint: '',
      signed_in: false,
    });
    onAuthenticated(profile);
  };

  return (
    <div
      className="h-full min-h-full flex flex-col items-center text-white px-6 relative overflow-hidden drag-region"
      style={{ background: 'rgba(10, 10, 12, 0.96)' }}
    >
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background: `radial-gradient(ellipse 500px 350px at 50% 30%, ${currentTheme.from}12, transparent)`,
        }}
      />

      <div className="flex-[0.8]" />

      <div className="w-full max-w-[340px] relative z-10 no-drag">
        <div className="flex justify-center mb-5">
          <VibnLogo size={56} />
        </div>

        <div className="flex justify-center mb-3">
          <Tabs
            tabs={[
              { id: 'sign-in', label: 'Sign In' },
              { id: 'sign-up', label: 'Sign Up' },
            ]}
            activeTab={mode}
            onTabChange={id => {
              setMode(id as 'sign-in' | 'sign-up');
              setError('');
            }}
            className="bg-black/35 rounded-lg p-1.5 border border-white/5 border-b-black/50 shadow-[inset_0_1px_3px_rgba(0,0,0,0.4)]"
          />
        </div>

        <div className="rounded-2xl p-4 bg-black/35 border border-white/5 border-t-white/10 border-b-black/50 shadow-[inset_0_1px_3px_rgba(0,0,0,0.4)]">
          <div className="mb-5 text-center">
            <AnimatePresence mode="wait">
              <motion.div
                key={mode}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
              >
                <h1 className="text-base font-semibold tracking-tight mb-0.5">
                  {mode === 'sign-in' ? 'Welcome back' : 'Create your account'}
                </h1>
                <p className="text-xs text-neutral-500">
                  {mode === 'sign-in' ? 'Sign in to continue to Vibn' : 'Get started with Vibn'}
                </p>
              </motion.div>
            </AnimatePresence>
          </div>

          {error && (
            <div className="mb-4 rounded-lg bg-red-500/10 border border-red-500/30 px-3.5 py-2.5 text-xs text-red-400">
              {error}
            </div>
          )}

          <form
            onSubmit={mode === 'sign-in' ? handleSignIn : handleSignUp}
            autoComplete="off"
          >
            <AnimatePresence initial={false}>
              {mode === 'sign-up' && (
                <motion.div
                  key="name-fields"
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  transition={{ duration: 0.25, ease: [0.25, 0.1, 0.25, 1] }}
                  className="overflow-hidden"
                >
                  <div className="grid grid-cols-2 gap-2.5 mb-2.5">
                    <div>
                      <label className="block text-[11px] font-medium text-neutral-400 mb-1">
                        First Name
                      </label>
                      <input
                        type="text"
                        value={firstName}
                        onChange={e => setFirstName(e.target.value)}
                        required
                        className={inputClassName}
                        style={inputStyle}
                        placeholder="John"
                      />
                    </div>
                    <div>
                      <label className="block text-[11px] font-medium text-neutral-400 mb-1">
                        Last Name
                      </label>
                      <input
                        type="text"
                        value={lastName}
                        onChange={e => setLastName(e.target.value)}
                        required
                        className={inputClassName}
                        style={inputStyle}
                        placeholder="Doe"
                      />
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <div className="mb-2.5">
              <label className="block text-[11px] font-medium text-neutral-400 mb-1">Email</label>
              <input
                type="text"
                value={email}
                onChange={e => setEmail(e.target.value)}
                required
                autoComplete="off"
                data-1p-ignore
                data-lpignore="true"
                className={inputClassName}
                style={inputStyle}
                placeholder="you@example.com"
              />
            </div>

            <div className="mb-2.5">
              <label className="block text-[11px] font-medium text-neutral-400 mb-1">Password</label>
              <input
                type="password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
                minLength={mode === 'sign-up' ? 8 : 1}
                autoComplete="new-password"
                data-1p-ignore
                data-lpignore="true"
                className={inputClassName}
                style={inputStyle}
                placeholder="••••••••"
              />
              <AnimatePresence>
                {mode === 'sign-up' && (
                  <motion.p
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
                    className="mt-1 text-[10px] text-neutral-600 overflow-hidden"
                  >
                    At least 8 characters
                  </motion.p>
                )}
              </AnimatePresence>
            </div>

            <label className="flex items-center gap-2 mb-2.5 text-[11px] text-neutral-500">
              <input
                type="checkbox"
                checked={remember}
                onChange={e => setRemember(e.target.checked)}
              />
              Remember me on this Mac
            </label>

            <div className="flex justify-end pt-1">
              <button
                type="submit"
                disabled={loading}
                className={cn(
                  'grid h-[34px] text-sm font-semibold rounded-md',
                  'border text-white no-drag whitespace-nowrap overflow-hidden',
                  'opacity-80 hover:opacity-100 disabled:opacity-40 disabled:cursor-not-allowed',
                  'transition-all duration-250 ease-in-out',
                )}
                style={{
                  background:
                    'linear-gradient(to bottom right, rgba(148, 85, 230, 0.35), rgba(88, 28, 135, 0.2))',
                  borderTopColor: 'rgba(167, 139, 250, 0.4)',
                  borderBottomColor: 'rgba(0, 0, 0, 0.35)',
                  borderLeftColor: 'transparent',
                  borderRightColor: 'transparent',
                  width: loading
                    ? mode === 'sign-in'
                      ? 116
                      : 156
                    : mode === 'sign-in'
                      ? 78
                      : 136,
                }}
              >
                <AnimatePresence mode="wait" initial={false}>
                  <motion.span
                    key={loading ? `${mode}-loading` : mode}
                    className="flex items-center justify-center px-3.5"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1, transition: { duration: 0.15, delay: 0.06 } }}
                    exit={{ opacity: 0, transition: { duration: 0.08 } }}
                  >
                    {loading
                      ? mode === 'sign-in'
                        ? 'Signing in...'
                        : 'Creating account...'
                      : mode === 'sign-in'
                        ? 'Sign In'
                        : 'Create Account'}
                  </motion.span>
                </AnimatePresence>
              </button>
            </div>
          </form>
        </div>

        <p className="mt-5 text-center text-[11px] text-neutral-600 leading-relaxed">
          By continuing, you agree to the{' '}
          <span className="text-neutral-500 hover:text-neutral-400 cursor-pointer transition-colors">
            Terms of Service
          </span>{' '}
          and{' '}
          <span className="text-neutral-500 hover:text-neutral-400 cursor-pointer transition-colors">
            Privacy Policy
          </span>
        </p>

        <p className="mt-4 text-center">
          <button
            type="button"
            onClick={handleSkip}
            className="text-[11px] text-neutral-500 hover:text-neutral-400 transition-colors underline underline-offset-2"
          >
            Use without account
          </button>
        </p>
      </div>

      <div className="flex-1" />
    </div>
  );
}
