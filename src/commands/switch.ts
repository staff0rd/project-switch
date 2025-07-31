import inquirer from 'inquirer';
import chalk from 'chalk';
import { ConfigManager } from '../config';

export async function switchCommand(): Promise<void> {
  const configManager = new ConfigManager();
  const projects = configManager.getProjects();
  const currentProject = configManager.getCurrentProject();

  if (projects.length === 0) {
    console.log(chalk.yellow('No projects found. Use "add" command to add a project.'));
    return;
  }

  const choices = projects.map(project => ({
    name: project.name === currentProject 
      ? chalk.green(`▶ ${project.name} (current)`)
      : `  ${project.name}`,
    value: project.name,
    short: project.name
  }));

  try {
    const { selectedProject } = await inquirer.prompt([
      {
        type: 'list',
        name: 'selectedProject',
        message: 'Select a project:',
        choices,
        default: currentProject,
        loop: false
      }
    ]);

    if (selectedProject !== currentProject) {
      configManager.setCurrentProject(selectedProject);
      console.log(chalk.green(`Switched to project: ${selectedProject}`));
    } else {
      console.log(chalk.blue(`Already on project: ${selectedProject}`));
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