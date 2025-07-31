import * as fs from "fs";
import * as yaml from "js-yaml";
import * as os from "os";
import * as path from "path";

export interface ProjectCommand {
  key: string;
  url?: string;
  browser?: string;
}

export interface Project {
  name: string;
  path?: string;
  description?: string;
  browser?: string;
  commands?: ProjectCommand[];
}

export interface Config {
  currentProject?: string;
  defaultBrowser?: string;
  projects: Project[];
}

const CONFIG_PATH = path.join(os.homedir(), ".project-switch.yml");

export class ConfigManager {
  private config: Config;

  constructor() {
    this.config = this.loadConfig();
  }

  private loadConfig(): Config {
    try {
      if (fs.existsSync(CONFIG_PATH)) {
        const fileContents = fs.readFileSync(CONFIG_PATH, "utf8");
        return (yaml.load(fileContents) as Config) || { projects: [] };
      }
    } catch (error) {
      console.error("Error loading config:", error);
    }
    return { projects: [] };
  }

  private saveConfig(): void {
    try {
      const yamlStr = yaml.dump(this.config);
      fs.writeFileSync(CONFIG_PATH, yamlStr, "utf8");
    } catch (error) {
      console.error("Error saving config:", error);
      throw error;
    }
  }

  getProjects(): Project[] {
    return this.config.projects;
  }

  getCurrentProject(): string | undefined {
    return this.config.currentProject;
  }

  setCurrentProject(projectName: string): void {
    const project = this.config.projects.find((p) => p.name === projectName);
    if (!project) {
      throw new Error(`Project "${projectName}" not found`);
    }
    this.config.currentProject = projectName;
    this.saveConfig();
  }

  addProject(project: Project): void {
    const existing = this.config.projects.find((p) => p.name === project.name);
    if (existing) {
      throw new Error(`Project "${project.name}" already exists`);
    }
    this.config.projects.push(project);
    if (!this.config.currentProject) {
      this.config.currentProject = project.name;
    }
    this.saveConfig();
  }

  projectExists(name: string): boolean {
    return this.config.projects.some((p) => p.name === name);
  }

  getProject(name: string): Project | undefined {
    return this.config.projects.find((p) => p.name === name);
  }

  getProjectCommand(
    projectName: string,
    commandKey: string
  ): ProjectCommand | undefined {
    const project = this.getProject(projectName);
    if (!project || !project.commands) {
      return undefined;
    }
    return project.commands.find((c) => c.key === commandKey);
  }

  getDefaultBrowser(): string {
    return this.config.defaultBrowser || "firefox";
  }
}
