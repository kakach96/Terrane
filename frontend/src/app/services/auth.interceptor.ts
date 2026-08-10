import { Injectable } from '@angular/core';
import { HttpInterceptor, HttpRequest, HttpHandler, HttpEvent } from '@angular/common/http';
import { Observable } from 'rxjs';
import { AuthService } from './auth.service';
import { detectLanguage } from './language.service';

@Injectable()
export class AuthInterceptor implements HttpInterceptor {
  constructor(private authService: AuthService) {}

  intercept(req: HttpRequest<unknown>, next: HttpHandler): Observable<HttpEvent<unknown>> {
    const token = this.authService.getToken();
    const headers: Record<string, string> = {
      'Accept-Language': detectLanguage(),
    };
    if (token && !req.url.includes('/auth/login')) {
      headers['Authorization'] = `Bearer ${token}`;
    }
    req = req.clone({ setHeaders: headers });
    return next.handle(req);
  }
}
