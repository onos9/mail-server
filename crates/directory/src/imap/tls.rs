use std::time::Duration;

use rustls::ServerName;
use smtp_proto::IntoString;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_rustls::{client::TlsStream, TlsConnector};

use super::{ImapClient, ImapError};

impl ImapClient<TcpStream> {
    async fn start_tls(
        mut self,
        tls_connector: &TlsConnector,
        tls_hostname: &str,
    ) -> Result<ImapClient<TlsStream<TcpStream>>, ImapError> {
        let line = tokio::time::timeout(self.timeout, async {
            self.write(b"C7 STARTTLS\r\n").await?;

            self.read_line().await
        })
        .await
        .map_err(|_| ImapError::Timeout)??;

        if matches!(line.get(..5), Some(b"C7 OK")) {
            self.into_tls(tls_connector, tls_hostname).await
        } else {
            Err(ImapError::InvalidResponse(line.into_string()))
        }
    }

    async fn into_tls(
        self,
        tls_connector: &TlsConnector,
        tls_hostname: &str,
    ) -> Result<ImapClient<TlsStream<TcpStream>>, ImapError> {
        tokio::time::timeout(self.timeout, async {
            Ok(ImapClient {
                stream: tls_connector
                    .connect(
                        ServerName::try_from(tls_hostname)
                            .map_err(|_| ImapError::TLSInvalidName)?,
                        self.stream,
                    )
                    .await?,
                timeout: self.timeout,
                mechanisms: self.mechanisms,
            })
        })
        .await
        .map_err(|_| ImapError::Timeout)?
    }
}

impl ImapClient<TlsStream<TcpStream>> {
    pub async fn connect(
        addr: impl ToSocketAddrs,
        timeout: Duration,
        tls_connector: &TlsConnector,
        tls_hostname: &str,
        tls_implicit: bool,
    ) -> Result<Self, ImapError> {
        let mut client: ImapClient<TcpStream> = tokio::time::timeout(timeout, async {
            match TcpStream::connect(addr).await {
                Ok(stream) => Ok(ImapClient {
                    stream,
                    timeout,
                    mechanisms: 0,
                }),
                Err(err) => Err(ImapError::Io(err)),
            }
        })
        .await
        .map_err(|_| ImapError::Timeout)??;

        if tls_implicit {
            let mut client = client.into_tls(tls_connector, tls_hostname).await?;
            client.expect_greeting().await?;
            Ok(client)
        } else {
            client.expect_greeting().await?;
            client.start_tls(tls_connector, tls_hostname).await
        }
    }
}