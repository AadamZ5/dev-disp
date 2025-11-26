use dev_disp_core::client::ScreenTransport;

pub struct WsTransport {}

impl ScreenTransport for WsTransport {
    fn initialize<'s>(
        &'s mut self,
    ) -> dev_disp_core::util::PinnedFuture<'s, Result<(), dev_disp_core::client::TransportError>>
    {
        todo!()
    }

    fn get_display_config<'s>(
        &'s mut self,
    ) -> dev_disp_core::util::PinnedFuture<
        's,
        Result<dev_disp_core::host::DisplayParameters, dev_disp_core::client::TransportError>,
    > {
        todo!()
    }

    fn send_screen_data<'s, 'a>(
        &'s mut self,
        data: &'a [u8],
    ) -> dev_disp_core::util::PinnedFuture<'s, Result<(), dev_disp_core::client::TransportError>>
    {
        todo!()
    }
}
