use block2::StackBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

use crate::microphone::MicrophoneStatus;

pub fn request_or_status() -> MicrophoneStatus {
    let media_type =
        unsafe { AVMediaTypeAudio }.expect("AVMediaTypeAudio constant missing at runtime");

    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };

    match status {
        AVAuthorizationStatus::Authorized => MicrophoneStatus::Authorized,
        AVAuthorizationStatus::NotDetermined => {
            let block = StackBlock::new(|_granted: Bool| {});
            unsafe {
                AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &block);
            }
            MicrophoneStatus::Pending
        }
        _ => MicrophoneStatus::Denied,
    }
}
