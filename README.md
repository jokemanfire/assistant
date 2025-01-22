# Real-Time Voice Assistant
## Project Overview
This project implements a real-time voice assistant that captures voice data through a microphone and converts it into text. Then, the text is sent to a dialogue model to generate a response, and finally, the response text is converted back into voice and played out.
Main Functional Modules
### 1.Speech to Text
This module is responsible for capturing audio data from the microphone and sending it to a speech-to-text API to convert the audio into text. It uses libraries like cpal to access the microphone device and reqwest to interact with the API.
### 2.Dialogue Model
The dialogue model uses the wasmedge to run the ggml llama model. It takes the converted text from the speech-to-text module as input, processes it through the model, and generates a response.
### 3.Text to Speech
This module takes the response from the dialogue model and sends it to a text-to-speech API to convert the text into audio data.
### 4.Audio Playback
The main program starts an audio stream from the microphone, captures the audio data, and sends it to the speech-to-text API. Then, the converted text is sent to the dialogue model API, the response is obtained, and the response text is converted back into voice and played.
# Running the Project
Ensure all dependencies are installed.
Replace the API keys with your actual keys.

cargo run

# Notes
Make sure there is an available microphone device on your system.
Adjust the audio data processing logic according to the actual situation, especially the buffering and chunk processing parts.
The specific implementation of playing audio needs to be adjusted according to the audio playback library you choose.