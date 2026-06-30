/**
 * Web Authentication Context
 * 
 * Replaces the Tauri-based authentication with REST API calls.
 * Manages JWT tokens, user state, and authentication flows.
 */

'use client';

import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { apiClient, User } from '@/lib/apiClient';

interface AuthContextType {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, fullName: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isInitialized, setIsInitialized] = useState(false);

  // Initialize auth state on mount
  useEffect(() => {
    const initAuth = async () => {
      const token = localStorage.getItem('auth_token');
      if (token) {
        apiClient.setToken(token);
        try {
          const userData = await apiClient.auth.me();
          setUser(userData);
        } catch (error) {
          // Token expired or invalid
          localStorage.removeItem('auth_token');
          apiClient.setToken(null);
        }
      }
      setIsLoading(false);
      setIsInitialized(true);
    };

    initAuth();
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    setIsLoading(true);
    try {
      const { user: userData, token } = await apiClient.auth.login({ email, password });
      apiClient.setToken(token);
      localStorage.setItem('auth_token', token);
      setUser(userData);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const register = useCallback(async (email: string, password: string, fullName: string) => {
    setIsLoading(true);
    try {
      const { user: userData, token } = await apiClient.auth.register({ 
        email, 
        password, 
        full_name: fullName 
      });
      apiClient.setToken(token);
      localStorage.setItem('auth_token', token);
      setUser(userData);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const logout = useCallback(async () => {
    setIsLoading(true);
    try {
      await apiClient.auth.logout();
    } catch (error) {
      // Ignore logout errors
      console.warn('Logout error:', error);
    } finally {
      apiClient.setToken(null);
      localStorage.removeItem('auth_token');
      setUser(null);
      setIsLoading(false);
    }
  }, []);

  const refreshUser = useCallback(async () => {
    try {
      const userData = await apiClient.auth.me();
      setUser(userData);
    } catch (error) {
      // Token might be expired
      apiClient.setToken(null);
      localStorage.removeItem('auth_token');
      setUser(null);
      throw error;
    }
  }, []);

  const changePassword = useCallback(async (currentPassword: string, newPassword: string) => {
    await apiClient.auth.changePassword({ current_password: currentPassword, new_password: newPassword });
  }, []);

  return (
    <AuthContext.Provider value={{
      user,
      isLoading,
      isAuthenticated: !!user,
      login,
      register,
      logout,
      refreshUser,
      changePassword,
    }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}