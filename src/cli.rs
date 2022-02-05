use clap::{Parser, Subcommand, AppSettings};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Logs in to AWS
    Login {
        /// Name of the environment to connect to (or profile - to use!)
        #[clap(short, long, alias = "profile", short_alias = 'p')]
        environment: Option<String>,
     },
    /// Execute bash in an ECS container
    Execute {
        /// Name of the environment to connect to (or profile - to use!)
        #[clap(short, long, alias = "profile", short_alias = 'p')]
        environment: Option<String>,
    
        /// Name of the container to connect to
        #[clap(long, visible_alias = "con")]
        container: Option<String>,
    
        /// Name of the cluster to connect to
        #[clap(short, long)]
        cluster: Option<String>,
    
        /// Name of the region to connect to
        #[clap(short, long)]
        region: Option<String>,
    
        // The ECS task to connect to
        task: Option<String>
    }
}
