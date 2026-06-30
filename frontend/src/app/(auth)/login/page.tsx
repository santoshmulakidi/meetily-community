/**
 * Login Page - Web Version
 * 
 * Handles user authentication for the web app.
 */

'use client';

import { useState } from 'react';
import { motion } from 'framer-motion';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import { useAuth } from '@/contexts/AuthContext';
import { Eye, EyeOff, Loader2, Mail, Lock, User, AlertCircle } from 'lucide-react';

export default function LoginPage() {
  const router = useRouter();
  const { login, register, isLoading, isAuthenticated } = useAuth();
  const [isLogin, setIsLogin] = useState(true);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [fullName, setFullName] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);

  // Redirect if already authenticated
  // Note: In a real app, you'd use a useEffect with router.push
  // but we handle it in the component body for SSR compatibility

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitError(null);
    setError(null);

    try {
      if (isLogin) {
        await login(email, password);
      } else {
        await register(email, password, fullName);
      }
      router.push('/');
      router.refresh();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Authentication failed';
      setSubmitError(message);
    }
  };

  const validateForm = () => {
    if (!email || !password) {
      setError('Please fill in all fields');
      return false;
    }
    if (!isLogin && !fullName) {
      setError('Please enter your full name');
      return false;
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return false;
    }
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      setError('Please enter a valid email address');
      return false;
    }
    return true;
  };

  const handleSubmitWithValidation = (e: React.FormEvent) => {
    if (validateForm()) {
      handleSubmit(e);
    }
  };

  // Toggle between login and register
  const toggleMode = () => {
    setIsLogin(prev => !prev);
    setError(null);
    setSubmitError(null);
    setFullName('');
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 py-12 px-4 sm:px-6 lg:px-8">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
        className="max-w-md w-full space-y-8"
      >
        <div className="text-center">
          <Link href="/" className="inline-flex items-center space-x-2">
            <svg className="h-12 w-12 text-indigo-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
            </svg>
            <span className="text-2xl font-bold text-gray-900">Meetily</span>
          </Link>
          <h2 className="mt-6 text-3xl font-extrabold text-gray-900">
            {isLogin ? 'Sign in to your account' : 'Create your account'}
          </h2>
          <p className="mt-2 text-sm text-gray-600">
            {isLogin 
              ? "Don't have an account? " 
              : 'Already have an account? '}
            <button
              onClick={toggleMode}
              className="font-medium text-indigo-600 hover:text-indigo-500"
            >
              {isLogin ? 'Sign up' : 'Sign in'}
            </button>
          </p>
        </div>

        <form className="mt-8 space-y-6" onSubmit={handleSubmitWithValidation}>
          {submitError && (
            <motion.div
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center p-3 text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg"
            >
              <AlertCircle className="mr-2 h-5 w-5 flex-shrink-0" />
              {submitError}
            </motion.div>
          )}

          {!isLogin && (
            <div className="relative">
              <User className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-gray-400" />
              <input
                id="fullName"
                name="fullName"
                type="text"
                autoComplete="name"
                required
                value={fullName}
                onChange={(e) => setFullName(e.target.value)}
                placeholder="Full name"
                className="appearance-none relative block w-full px-3 py-2 pl-10 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-lg focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 focus:z-10 sm:text-sm"
              />
            </div>
          )}

          <div className="relative">
            <Mail className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-gray-400" />
            <input
              id="email"
              name="email"
              type="email"
              autoComplete={isLogin ? 'email' : 'email'}
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Email address"
              className="appearance-none relative block w-full px-3 py-2 pl-10 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-lg focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 focus:z-10 sm:text-sm"
            />
          </div>

          <div className="relative">
            <Lock className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-gray-400" />
            <input
              id="password"
              name="password"
              type={showPassword ? 'text' : 'password'}
              autoComplete={isLogin ? 'current-password' : 'new-password'}
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              className="appearance-none relative block w-full px-3 py-2 pl-10 pr-10 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-lg focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 focus:z-10 sm:text-sm"
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute inset-y-0 right-0 pr-3 flex items-center text-gray-400 hover:text-gray-600"
            >
              {showPassword ? <EyeOff className="h-5 w-5" /> : <Eye className="h-5 w-5" />}
            </button>
          </div>

          <div>
            <button
              type="submit"
              disabled={isLoading}
              className="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-lg text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isLoading ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : null}
              {isLogin ? 'Sign in' : 'Create account'}
            </button>
          </div>
        </form>

        <div className="text-center text-sm text-gray-500">
          <p>By continuing, you agree to our</p>
          <div className="flex justify-center space-x-4 mt-1">
            <Link href="/terms" className="text-indigo-600 hover:text-indigo-500">Terms of Service</Link>
            <Link href="/privacy" className="text-indigo-600 hover:text-indigo-500">Privacy Policy</Link>
          </div>
        </div>
      </motion.div>
    </div>
  );
}