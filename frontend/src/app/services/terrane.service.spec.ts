import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting, HttpTestingController } from '@angular/common/http/testing';
import { TerraneService } from './terrane.service';

describe('TerraneService', () => {
  let service: TerraneService;
  let httpMock: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [provideHttpClient(), provideHttpClientTesting()],
    });
    service = TestBed.inject(TerraneService);
    httpMock = TestBed.inject(HttpTestingController);
  });

  afterEach(() => httpMock.verify());

  it('requests layers under the /terrane API context and unwraps ApiResponse.data', () => {
    const result: unknown[] = [];
    service.getLayers().subscribe((layers) => result.push(...layers));
    const req = httpMock.expectOne('/terrane/layers');
    expect(req.request.method).toBe('GET');
    req.flush({ success: true, data: [{ name: 'cities' }] });
    expect(result).toEqual([{ name: 'cities' }]);
  });

  it('requests monitor stats, health and catalog from the monitor endpoints', () => {
    const collected: unknown[] = [];
    service.getMonitorStats().subscribe((s) => collected.push(s));
    service.getMonitorHealth().subscribe((h) => collected.push(h));
    service.getCatalogStats().subscribe((c) => collected.push(c));

    const stats = httpMock.expectOne('/terrane/monitor/stats');
    expect(stats.request.method).toBe('GET');
    stats.flush({
      uptime_seconds: 1,
      total_requests: 0,
      total_errors: 0,
      error_rate: 0,
      requests_per_second: 0,
      endpoints: {},
      methods: {},
      status_codes: {},
      system: {
        version: '0.1.0',
        rust_version: 'unknown',
        os: 'windows',
        hostname: 'h',
        cpu_cores: 1,
        memory_mb: 1,
      },
    });

    const health = httpMock.expectOne('/terrane/monitor/health');
    expect(health.request.method).toBe('GET');
    health.flush({ status: 'ready', checks: [{ name: 'metadata_store', status: 'ok' }] });

    const catalog = httpMock.expectOne('/terrane/monitor/catalog');
    expect(catalog.request.method).toBe('GET');
    catalog.flush({ workspaces: 1, data_sources: 3, layers: 4, layer_groups: 0 });

    expect(collected.length).toBe(3);
  });

  it('lists and cancels tile seed jobs via /tiles/seed', () => {
    const jobs: unknown[] = [];
    service.getSeedJobs().subscribe((j) => jobs.push(...j));
    const list = httpMock.expectOne('/terrane/tiles/seed');
    expect(list.request.method).toBe('GET');
    list.flush({
      success: true,
      data: [{ id: 'job1', layer: 'cities', gridset: 'EPSG:4326', z_min: 0, z_max: 5, format: 'png', status: 'Running', total: 100, done: 10, created_at: '', updated_at: '' }],
    });
    expect(jobs.length).toBe(1);

    service.cancelSeedJob('job1').subscribe();
    const cancel = httpMock.expectOne('/terrane/tiles/seed/job1');
    expect(cancel.request.method).toBe('DELETE');
    cancel.flush({ success: true, data: { job: 'job1', message: 'cancel requested' } });
  });

  it('posts a seed job request to /tiles/seed', () => {
    service.createSeedJob({ layer: 'cities', z_min: 0, z_max: 3 }).subscribe();
    const req = httpMock.expectOne('/terrane/tiles/seed');
    expect(req.request.method).toBe('POST');
    expect(req.request.body).toEqual({ layer: 'cities', z_min: 0, z_max: 3 });
    req.flush({ success: true, data: { id: 'job2' } });
  });
});
