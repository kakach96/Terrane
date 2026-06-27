import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, BehaviorSubject } from 'rxjs';
import { map, tap } from 'rxjs/operators';
import { ApiResponse } from '../models/geoserver.models';

export interface AuthUser {
  username: string;
  role: string;
  token: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly apiUrl = '/geoserver/auth';
  private currentUserSubject = new BehaviorSubject<AuthUser | null>(null);
  currentUser$ = this.currentUserSubject.asObservable();

  constructor(private http: HttpClient) {
    this.loadStoredUser();
  }

  private loadStoredUser(): void {
    const stored = localStorage.getItem('geoserver_user');
    if (stored) {
      try {
        this.currentUserSubject.next(JSON.parse(stored));
      } catch {
        localStorage.removeItem('geoserver_user');
      }
    }
  }

  login(username: string, password: string): Observable<AuthUser> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/login`, { username, password })
      .pipe(map(res => res.data as AuthUser))
      .pipe(tap(user => {
        localStorage.setItem('geoserver_user', JSON.stringify(user));
        localStorage.setItem('geoserver_token', user.token);
        this.currentUserSubject.next(user);
      }));
  }

  logout(): void {
    localStorage.removeItem('geoserver_user');
    localStorage.removeItem('geoserver_token');
    this.currentUserSubject.next(null);
  }

  getToken(): string | null {
    return localStorage.getItem('geoserver_token');
  }

  isLoggedIn(): boolean {
    return !!this.getToken();
  }

  isAdmin(): boolean {
    const user = this.currentUserSubject.value;
    return user?.role === 'admin';
  }

  getCurrentUser(): AuthUser | null {
    return this.currentUserSubject.value;
  }
}
