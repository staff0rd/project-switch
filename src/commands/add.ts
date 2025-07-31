import inquirer from 'inquirer';
import chalk from 'chalk';
import { ConfigManager, Project } from '../config';

export async function addCommand(name?: string): Promise<void> {
  const configManager = new ConfigManager();

  try {
    let projectName = name;

    if (!projectName) {
      const { inputName } = await inquirer.prompt([
        {
          type: 'input',
          name: 'inputName',
          message: 'Enter project name:',
          validate: (input: string) => {
            if (!input.trim()) {
              return 'Project name cannot be empty';
            }
            if (configManager.projectExists(input.trim())) {
              return `Project "${input.trim()}" already exists`;
            }
            return true;
          }
        }
      ]);
      projectName = inputName.trim();
    } else {
      projectName = projectName.trim();
      if (configManager.projectExists(projectName)) {
        console.error(chalk.red(`Project "${projectName}" already exists`));
        return;
      }
    }

    const project: Project = {
      name: projectName!
    };

    configManager.addProject(project);
    console.log(chalk.green(`Project "${projectName}" added successfully!`));

    const projects = configManager.getProjects();
    if (projects.length === 1) {
      console.log(chalk.blue(`"${projectName}" is now the current project.`));
    }
  } catch (error: any) {
    if (error.isTtyError) {
      console.error(chalk.red('Prompt couldn\'t be rendered in the current environment'));
    } else if (error instanceof Error) {
      console.error(chalk.red('An error occurred:'), error.message);
    } else {
      console.error(chalk.red('An unknown error occurred'));
    }
  }
}