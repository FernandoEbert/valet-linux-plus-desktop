// Mirror of src-tauri/src/valet/model.rs (valet state schema v1 + app fields).

export type Source = "cli" | "files";
export type SiteType = "parked" | "linked" | "proxy" | "other";

export interface CliPhp {
  path: string | null;
  binary: string | null;
  version: string | null;
}

export interface PhpInfo {
  global: string | null;
  cli: CliPhp;
  fallback_binary: string | null;
  installed: string[];
}

export interface Site {
  name: string;
  domain: string;
  url: string;
  path: string | null;
  type: SiteType;
  secure: boolean;
  php: string | null;
  isolated_php: string | null;
  phprc_php: string | null;
  custom_nginx: boolean;
  proxy_host: string | null;
}

export interface Service {
  name: string;
  unit: string;
  /** true running, false stopped/failed, null not installed */
  active: boolean | null;
}

export interface ValetState {
  schema_version: number;
  valet_version: string | null;
  home: string;
  domain: string;
  port: string;
  https_port: string;
  php: PhpInfo;
  paths: string[];
  sites: Site[];
  services: Service[];
  source: Source;
  valet_path: string | null;
  warnings: string[];
}
