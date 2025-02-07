# Real-Time Voice Assistant

## Project Overview

This project implements a real-time voice assistant that seamlessly captures voice data through a microphone, converts it into text, processes the text using a dialogue model to generate a response, and finally converts the response text back into voice for playback.

## Design
![PlantUML Diagram](design.puml)

### Main Functional Modules

#### 1. Speech to Text
This module is responsible for capturing audio data from the microphone and sending it to a speech-to-text API to convert the audio into text. It leverages powerful libraries such as `cpal` to access the microphone device and `reqwest` to interact with the API smoothly.

- **Microphone Access**: `cpal` provides a cross-platform way to access audio input devices, allowing the application to capture audio data from the microphone in real-time.
- **API Interaction**: `reqwest` is used to send HTTP requests to the speech-to-text API, ensuring reliable communication and efficient data transfer.

#### 2. Dialogue Model
The dialogue model utilizes `wasmedge` to run the `ggml llama` model. It takes the converted text from the speech-to-text module as input, processes it through the model, and generates a meaningful response.

- **Model Execution**: `wasmedge` enables the efficient execution of the `ggml llama` model, providing high-performance inference capabilities.
- **Text Processing**: The model analyzes the input text, understands the user's intent, and generates an appropriate response based on its training.

#### 3. Text to Speech
This module takes the response from the dialogue model and sends it to a text-to-speech API to convert the text into audio data.

- **API Integration**: Similar to the speech-to-text module, `reqwest` is used to interact with the text-to-speech API, sending the response text and receiving the corresponding audio data.

#### 4. Audio Playback
The main program initiates an audio stream from the microphone, captures the audio data, and sends it to the speech-to-text API. Then, the converted text is sent to the dialogue model API, the response is obtained, and the response text is converted back into voice and played.

- **Audio Streaming**: The application continuously captures audio data from the microphone in real-time, ensuring a seamless user experience.
- **Playback Library**: Depending on the audio playback library chosen, the audio data is played back to the user, providing an immersive voice interaction experience.

## Running the Project

### Prerequisites
- Ensure all dependencies are installed. The project relies on several libraries, including `cpal`, `reqwest`, `serde_json`, `tokio`, `rodio`, `ttrpc`, `prost-build`, and `ttrpc-codegen`. You can install them using `cargo` by running `cargo build` in the project directory.
- Replace the API keys with your actual keys. The project uses API keys to access the speech-to-text and text-to-speech APIs. You can obtain these keys from the respective API providers and replace the placeholder keys in the code.

### Steps
1. Clone the project repository to your local machine.
2. Navigate to the project directory.
3. Replace the API keys in the code with your actual keys.
4. Run the following command to start the application:
```sh
cargo run
```

## Notes
- **Microphone Availability**: Make sure there is an available microphone device on your system. If the microphone is not detected or not working properly, the speech-to-text module will not be able to capture audio data.
- **Audio Data Processing**: Adjust the audio data processing logic according to the actual situation, especially the buffering and chunk processing parts. Different microphones and audio devices may have different sample rates and formats, so you may need to adjust the code accordingly.
- **Audio Playback Implementation**: The specific implementation of playing audio needs to be adjusted according to the audio playback library you choose. The project currently uses `rodio` for audio playback, but you can choose other libraries based on your requirements.